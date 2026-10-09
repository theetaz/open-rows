// SPDX-License-Identifier: Apache-2.0
//! Crash safety (M1-02, NFR-REL-01): a process killed while it writes leaves
//! a store that opens, passes `PRAGMA integrity_check` and holds every
//! committed write, with derived data (the history search index) in step.
//!
//! The test re-runs its own test binary as the writer: `writer_child` does
//! nothing unless [`CHILD_ENV`] names a database, in which case it writes
//! in a loop until it is killed.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use openrows_store::{HistoryRecord, HistorySource, HistoryStatus, LocalStore, WorkspaceKey};

/// Database path for the writer child; unset in the normal test run.
const CHILD_ENV: &str = "OPENROWS_STORE_CRASH_WRITER";
/// Writes committed before the child reports that it is under way.
const WARM_UP: usize = 200;
const STATE: WorkspaceKey<'static> = WorkspaceKey {
    window: "main",
    part: "queryTabs",
};

fn record(index: usize) -> HistoryRecord {
    HistoryRecord {
        profile_id: "p1".to_owned(),
        database: Some("orders".to_owned()),
        started_at_ms: i64::try_from(index).unwrap_or(i64::MAX),
        duration_ms: Some(1),
        status: HistoryStatus::Ok,
        rows: Some(1),
        // Long enough that a write spans several pages.
        statement: format!("SELECT {index} /* {} */", "x".repeat(4096)),
        error_message: None,
        source: HistorySource::Editor,
    }
}

#[test]
fn writer_child() {
    let Ok(path) = std::env::var(CHILD_ENV) else {
        return;
    };
    let store = LocalStore::open(std::path::Path::new(&path)).unwrap();
    for index in 1.. {
        store.record_history(&record(index)).unwrap();
        let state = format!(r#"{{"written":{index}}}"#);
        store.save_workspace_state(STATE, &state, 0).unwrap();
        if index == WARM_UP {
            let mut out = std::io::stdout();
            writeln!(out, "writer-ready").unwrap();
            out.flush().unwrap();
        }
    }
}

/// Start the writer on `path` and kill it once it has committed `WARM_UP`
/// writes; `false` when it exited before that.
fn kill_writer_mid_loop(path: &std::path::Path) -> std::io::Result<bool> {
    let mut child = Command::new(std::env::current_exe()?)
        .args(["--exact", "writer_child", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let ready = child.stdout.take().is_some_and(|stdout| {
        BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .any(|line| line.contains("writer-ready"))
    });
    child.kill()?;
    child.wait()?;
    Ok(ready)
}

fn count(conn: &rusqlite::Connection, sql: &str) -> rusqlite::Result<i64> {
    conn.query_row(sql, [], |row| row.get(0))
}

#[test]
fn killed_writer_leaves_a_consistent_store() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("openrows.sqlite");
    drop(LocalStore::open(&path).unwrap());
    assert!(
        kill_writer_mid_loop(&path).unwrap(),
        "the writer exited before it was ready"
    );

    // Reopening recovers the WAL; the store must open and migrate as usual.
    let store = LocalStore::open(&path).unwrap();
    let state = store.workspace_state(STATE).unwrap().unwrap();
    drop(store);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    let rows = count(&conn, "SELECT count(*) FROM history").unwrap();
    assert!(rows >= i64::try_from(WARM_UP).unwrap());
    assert_eq!(
        count(&conn, "SELECT max(id) FROM history").unwrap(),
        rows,
        "a committed history row is missing"
    );
    assert_eq!(
        count(&conn, "SELECT count(*) FROM history_fts").unwrap(),
        rows,
        "the search index is out of step with history"
    );
    let written: serde_json::Value = serde_json::from_str(&state).unwrap();
    let written = written["written"].as_i64().unwrap();
    assert!(
        written == rows || written == rows - 1,
        "workspace state {written} does not match {rows} history rows"
    );
    let fts: String = conn
        .query_row(
            "SELECT 'ok' FROM history_fts WHERE history_fts MATCH 'SELECT' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fts, "ok");
}
