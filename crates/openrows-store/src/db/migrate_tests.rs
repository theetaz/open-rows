// SPDX-License-Identifier: Apache-2.0
//! Migration runner, down-step and per-version upgrade regressions.
use rusqlite::Connection;

use super::{MIGRATIONS, SCHEMA_VERSION, migrate, user_version};

/// `span_ms, min_value, max_value, sum_value, ok_count` of a monitor history row.
type RollupColumns = (i64, Option<f64>, Option<f64>, f64, i64);

/// The down step of every migration, in version order; the runner itself
/// only applies up steps.
const DOWN: &[&str] = &[
    include_str!("../../migrations/0001_init.down.sql"),
    include_str!("../../migrations/0002_change_sets.down.sql"),
    include_str!("../../migrations/0003_history_search.down.sql"),
    include_str!("../../migrations/0004_saved_queries.down.sql"),
    include_str!("../../migrations/0005_schema_snapshots.down.sql"),
    include_str!("../../migrations/0006_workspace_state.down.sql"),
    include_str!("../../migrations/0007_monitor_history.down.sql"),
    include_str!("../../migrations/0008_boards.down.sql"),
    include_str!("../../migrations/0009_monitor_rollups.down.sql"),
];
const DOWN_V3: &str = DOWN[2];

/// Every schema object, as `type name sql`, sorted.
fn schema(conn: &Connection) -> Vec<String> {
    let mut statement = conn
        .prepare(
            "SELECT type || ' ' || name || ' ' || coalesce(sql, '') FROM sqlite_master \
             WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
        )
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// Run the down steps from the current version to 0, newest first.
fn revert_all(conn: &Connection) {
    for (index, down) in DOWN.iter().enumerate().rev() {
        conn.execute_batch(down).unwrap();
        conn.pragma_update(None, "user_version", u32::try_from(index).unwrap())
            .unwrap();
    }
}

#[test]
fn every_migration_has_a_down_step() {
    assert_eq!(DOWN.len(), MIGRATIONS.len());
}

#[test]
fn migrations_go_up_down_and_up_again() {
    let dir = tempfile::tempdir().unwrap();
    let backups = dir.path().join("backups");
    let mut conn = Connection::open(dir.path().join("openrows.sqlite")).unwrap();
    migrate(&mut conn, MIGRATIONS, &backups).unwrap();
    let first = schema(&conn);
    assert!(
        first
            .iter()
            .any(|item| item.starts_with("table workspace_state"))
    );

    revert_all(&conn);
    assert_eq!(user_version(&conn).unwrap(), 0);
    assert!(schema(&conn).is_empty(), "down steps leave objects behind");

    migrate(&mut conn, MIGRATIONS, &backups).unwrap();
    assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);
    assert_eq!(schema(&conn), first);
}

fn history_objects(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE name LIKE 'history_fts%'",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn schema_version_matches_last_migration() {
    assert_eq!(
        MIGRATIONS.last().map(|step| step.version),
        Some(SCHEMA_VERSION)
    );
}

#[test]
fn upgrade_backs_up_then_applies_only_pending_steps() {
    let dir = tempfile::tempdir().unwrap();
    let backups = dir.path().join("backups");
    let mut conn = Connection::open(dir.path().join("openrows.sqlite")).unwrap();
    migrate(&mut conn, &MIGRATIONS[..2], &backups).unwrap();
    conn.execute("INSERT INTO meta VALUES ('marker', 'v1')", [])
        .unwrap();

    migrate(&mut conn, MIGRATIONS, &backups).unwrap();
    migrate(&mut conn, MIGRATIONS, &backups).unwrap();
    assert_eq!(user_version(&conn).unwrap(), SCHEMA_VERSION);

    let backup = Connection::open(backups.join("openrows.sqlite.v2")).unwrap();
    assert_eq!(user_version(&backup).unwrap(), 2);
    let marker: String = backup
        .query_row("SELECT value FROM meta WHERE key = 'marker'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(marker, "v1");
}

#[test]
fn monitor_rollups_turn_existing_samples_into_raw_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = Connection::open(dir.path().join("openrows.sqlite")).unwrap();
    migrate(&mut conn, &MIGRATIONS[..8], &dir.path().join("backups")).unwrap();
    conn.execute_batch(
        "INSERT INTO monitor_history VALUES ('p', 'db', 'sessions:active', 1, 4.5, 'ok'), \
         ('p', 'db', 'sessions:active', 2, NULL, 'gap')",
    )
    .unwrap();

    migrate(&mut conn, MIGRATIONS, &dir.path().join("backups")).unwrap();
    let rows: Vec<RollupColumns> = conn
        .prepare(
            "SELECT span_ms, min_value, max_value, sum_value, ok_count \
             FROM monitor_history ORDER BY collected_ms",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        [(0, Some(4.5), Some(4.5), 4.5, 1), (0, None, None, 0.0, 0)]
    );
}

#[test]
fn history_search_up_indexes_old_rows_and_down_reverts_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let mut conn = Connection::open(dir.path().join("openrows.sqlite")).unwrap();
    migrate(&mut conn, &MIGRATIONS[..2], &dir.path().join("backups")).unwrap();
    conn.execute(
        "INSERT INTO history (profile_id, started_at, status, statement, source) \
         VALUES ('p1', 1, 'ok', 'SELECT * FROM orders', 'editor')",
        [],
    )
    .unwrap();

    migrate(&mut conn, MIGRATIONS, &dir.path().join("backups")).unwrap();
    let indexed: i64 = conn
        .query_row(
            "SELECT count(*) FROM history_fts WHERE history_fts MATCH 'orders'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexed, 1, "rows recorded before v3 must be searchable");

    conn.execute_batch(DOWN_V3).unwrap();
    assert_eq!(history_objects(&conn), 0);
    conn.execute_batch(MIGRATIONS[2].sql).unwrap();
    assert!(history_objects(&conn) > 0);
}
