// SPDX-License-Identifier: Apache-2.0
//! Numbered, embedded schema migrations (docs/15 §6).
//!
//! The applied version is `PRAGMA user_version`. Pending migrations run in
//! one transaction after a `VACUUM INTO` backup of the existing database.

use std::path::Path;

use rusqlite::Connection;

use super::error::{StoreError, sqlite};

/// One schema step: target version and its SQL.
#[derive(Clone, Copy)]
pub(crate) struct Migration {
    pub(crate) version: u32,
    pub(crate) sql: &'static str,
}

/// Every migration shipped with this build, in ascending version order.
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../../migrations/0001_init.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../../migrations/0002_change_sets.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../../migrations/0003_history_search.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../../migrations/0004_saved_queries.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("../../migrations/0005_schema_snapshots.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("../../migrations/0006_workspace_state.sql"),
    },
    Migration {
        version: 7,
        sql: include_str!("../../migrations/0007_monitor_history.sql"),
    },
];

/// Newest local store schema version this build understands.
pub const SCHEMA_VERSION: u32 = 7;

pub(crate) fn user_version(conn: &Connection) -> Result<u32, StoreError> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(sqlite("read schema version"))
}

/// Bring `conn` up to the last of `migrations`, backing up into `backup_dir`
/// first when an older, non-empty schema is present.
pub(crate) fn migrate(
    conn: &mut Connection,
    migrations: &[Migration],
    backup_dir: &Path,
) -> Result<(), StoreError> {
    let current = user_version(conn)?;
    let latest = migrations.last().map_or(0, |step| step.version);
    if current > latest {
        return Err(StoreError::NewerSchema {
            found: current,
            supported: latest,
        });
    }
    if current == latest {
        return Ok(());
    }
    if current > 0 {
        backup(conn, backup_dir, current)?;
    }
    apply(conn, migrations, current, latest)
}

fn apply(
    conn: &mut Connection,
    migrations: &[Migration],
    current: u32,
    latest: u32,
) -> Result<(), StoreError> {
    let tx = conn.transaction().map_err(sqlite("begin migration"))?;
    for step in migrations.iter().filter(|step| step.version > current) {
        tx.execute_batch(step.sql).map_err(sqlite("migrate"))?;
        tx.pragma_update(None, "user_version", step.version)
            .map_err(sqlite("record schema version"))?;
    }
    tx.execute(
        "INSERT INTO meta (key, value) VALUES ('schema_version', ?1) \
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [latest.to_string()],
    )
    .map_err(sqlite("record schema version"))?;
    tx.commit().map_err(sqlite("commit migration"))
}

fn backup(conn: &Connection, backup_dir: &Path, version: u32) -> Result<(), StoreError> {
    std::fs::create_dir_all(backup_dir).map_err(|source| StoreError::Io {
        path: backup_dir.to_path_buf(),
        source,
    })?;
    let target = backup_dir.join(format!("openrows.sqlite.v{version}"));
    match std::fs::remove_file(&target) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(StoreError::Io {
                path: target,
                source,
            });
        }
    }
    let target_text = target.to_string_lossy();
    conn.execute("VACUUM INTO ?1", [target_text.as_ref()])
        .map_err(sqlite("backup before migration"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{MIGRATIONS, SCHEMA_VERSION, migrate, user_version};

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
}
