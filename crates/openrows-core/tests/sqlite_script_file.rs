// SPDX-License-Identifier: Apache-2.0
//! Running `.sql` files through the session actor and the real `SQLite`
//! driver: stop or continue on error, one transaction, safety checks and
//! files larger than one read chunk.
use openrows_core::safety::{Gate, SafetyPolicy, Verdict};
use openrows_core::script::ScriptCanceller;
use openrows_core::script_file::{
    FileEvent, FileOutcome, FileRun, FileStamp, FileSummary, run_file, scan_file,
};
use openrows_core::{QueryEvent, SessionManager};
use openrows_driver_api::Session;
use openrows_results::ResultStore;
use openrows_sql::Dialect;
use openrows_types::SessionId;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const POLICY: SafetyPolicy = SafetyPolicy {
    mode: openrows_types::safety::Safety::Unrestricted,
    dialect: Dialect::Sqlite,
};

/// Line 4 fails (no such table); the statements around it are fine.
const SCRIPT: &str = "CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT);\n\
    INSERT INTO t VALUES (1, 'a;b');\n\
    -- the next statement fails\n\
    INSERT INTO missing VALUES (2);\n\
    INSERT INTO t VALUES (3, 'c');\n\
    SELECT * FROM t;\n";

struct Fixture {
    store: Arc<ResultStore>,
    manager: SessionManager,
    session: SessionId,
    directory: tempfile::TempDir,
}

async fn fixture() -> TestResult<Fixture> {
    let store = Arc::new(ResultStore::default());
    let manager = SessionManager::new(store.clone());
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("db.sqlite");
    let session = manager
        .open(async move {
            let session: Box<dyn Session> =
                Box::new(openrows_driver_sqlite::open(&path, false, true)?);
            Ok(session)
        })
        .await?;
    Ok(Fixture {
        store,
        manager,
        session,
        directory,
    })
}

impl Fixture {
    fn script(&self, name: &str, text: &[u8]) -> TestResult<std::path::PathBuf> {
        let path = self.directory.path().join(name);
        std::fs::write(&path, text)?;
        Ok(path)
    }

    async fn run(&self, path: &Path, options: (bool, bool)) -> (FileSummary, Vec<FileEvent>) {
        let mut events = Vec::new();
        let run = FileRun {
            path,
            policy: POLICY,
            stop_on_error: options.0,
            transaction: options.1,
            stamp: None,
        };
        let cancel = ScriptCanceller::default();
        let summary = run_file(&self.manager, self.session, run, &cancel, |event| {
            events.push(event);
        })
        .await;
        (summary, events)
    }

    /// `SELECT count(*)` of `table`, or `None` when the table does not exist.
    async fn count(&self, table: &str) -> TestResult<Option<i64>> {
        let sql = format!("SELECT count(*) FROM {table}");
        let mut handle = self.manager.execute(self.session, sql)?;
        let mut result = None;
        while let Some(event) = handle.next_event().await {
            match event {
                QueryEvent::ResultBegin { result: id, .. } => result = Some(id),
                QueryEvent::Failed { .. } => return Ok(None),
                _ => {}
            }
        }
        Ok(result.and_then(|id| self.store.first_integer(&id.to_string())))
    }
}

fn errors(events: &[FileEvent]) -> Vec<(u64, usize, String)> {
    events
        .iter()
        .filter_map(|event| match event {
            FileEvent::Error(error) => Some((error.statement, error.line, error.preview.clone())),
            FileEvent::Progress(_) => None,
        })
        .collect()
}

#[tokio::test]
async fn continuing_after_an_error_runs_the_rest_and_reports_its_line() -> TestResult {
    let fixture = fixture().await?;
    let path = fixture.script("script.sql", SCRIPT.as_bytes())?;
    let (summary, events) = fixture.run(&path, (false, false)).await;
    assert_eq!(summary.outcome, FileOutcome::Failed);
    assert_eq!((summary.statements, summary.errors), (5, 1));
    assert_eq!(summary.rows_affected, 2);
    assert_eq!(
        errors(&events),
        [(
            3,
            4,
            "-- the next statement fails INSERT INTO missing VALUES (2)".to_owned()
        )]
    );
    assert_eq!(fixture.count("t").await?, Some(2));
    let Some(FileEvent::Progress(last)) = events.last() else {
        return Err("no final progress".into());
    };
    assert_eq!(last.bytes, SCRIPT.trim_end().len() as u64);
    assert_eq!(last.statements, 5);
    Ok(())
}

#[tokio::test]
async fn stopping_at_the_first_error_skips_the_rest() -> TestResult {
    let fixture = fixture().await?;
    let path = fixture.script("script.sql", SCRIPT.as_bytes())?;
    let (summary, _) = fixture.run(&path, (true, false)).await;
    assert_eq!(summary.outcome, FileOutcome::Failed);
    assert_eq!((summary.statements, summary.errors), (3, 1));
    assert!(!summary.rolled_back);
    assert_eq!(fixture.count("t").await?, Some(1));
    Ok(())
}

#[tokio::test]
async fn a_transaction_rolls_everything_back_on_error_and_commits_on_success() -> TestResult {
    let fixture = fixture().await?;
    let failing = fixture.script("failing.sql", SCRIPT.as_bytes())?;
    let (summary, _) = fixture.run(&failing, (false, true)).await;
    assert_eq!(summary.outcome, FileOutcome::Failed);
    assert!(summary.rolled_back);
    assert_eq!(fixture.count("t").await?, None);
    let fine = SCRIPT.replace("INSERT INTO missing VALUES (2);\n", "");
    let passing = fixture.script("passing.sql", fine.as_bytes())?;
    let (summary, _) = fixture.run(&passing, (true, true)).await;
    assert_eq!(summary.outcome, FileOutcome::Completed);
    assert!(!summary.rolled_back);
    assert_eq!(fixture.count("t").await?, Some(2));
    Ok(())
}

#[tokio::test]
async fn files_larger_than_a_read_chunk_stream_through() -> TestResult {
    let fixture = fixture().await?;
    let mut script = String::from("\u{feff}CREATE TABLE big (id INTEGER, note TEXT);\n");
    for id in 0..16_000 {
        writeln!(
            script,
            "INSERT INTO big VALUES ({id}, 'ünïcödé 😀 {}');",
            "x".repeat(90)
        )?;
    }
    assert!(script.len() > 2 * 1024 * 1024);
    let path = fixture.script("big.sql", script.as_bytes())?;
    let scan = scan_file(&path, POLICY, || false)?;
    assert_eq!(scan.statements, 16_001);
    let (summary, events) = fixture.run(&path, (true, false)).await;
    assert_eq!(summary.outcome, FileOutcome::Completed);
    assert_eq!(summary.statements, 16_001);
    assert_eq!(fixture.count("big").await?, Some(16_000));
    let Some(FileEvent::Progress(last)) = events.last() else {
        return Err("no final progress".into());
    };
    assert_eq!(last.bytes, script.trim_end().len() as u64);
    Ok(())
}

#[tokio::test]
async fn invalid_text_and_changed_files_stop_the_run() -> TestResult {
    let fixture = fixture().await?;
    let binary = fixture.script("binary.sql", b"SELECT 1;\nSELECT '\xff';\n")?;
    let (summary, _) = fixture.run(&binary, (true, false)).await;
    assert_eq!(summary.outcome, FileOutcome::Failed);
    let message = summary.message.unwrap_or_default();
    assert!(message.contains("not UTF-8"), "{message}");
    let path = fixture.script("script.sql", SCRIPT.as_bytes())?;
    let stamp = FileStamp::of(&path)?;
    std::fs::write(&path, "SELECT 2;")?;
    let run = FileRun {
        path: &path,
        policy: POLICY,
        stop_on_error: true,
        transaction: false,
        stamp: Some(stamp),
    };
    let summary = run_file(
        &fixture.manager,
        fixture.session,
        run,
        &ScriptCanceller::default(),
        drop,
    )
    .await;
    assert_eq!(summary.statements, 0);
    assert!(summary.message.unwrap_or_default().contains("changed"));
    Ok(())
}

#[tokio::test]
async fn a_cancelled_run_stops_before_the_next_statement() -> TestResult {
    let fixture = fixture().await?;
    let path = fixture.script("script.sql", SCRIPT.as_bytes())?;
    let cancel = ScriptCanceller::default();
    cancel.cancel();
    let run = FileRun {
        path: &path,
        policy: POLICY,
        stop_on_error: false,
        transaction: false,
        stamp: None,
    };
    let summary = run_file(&fixture.manager, fixture.session, run, &cancel, drop).await;
    assert_eq!(summary.outcome, FileOutcome::Cancelled);
    assert_eq!(summary.statements, 0);
    assert_eq!(fixture.count("t").await?, None);
    Ok(())
}

#[test]
fn the_safety_check_blocks_read_only_and_asks_before_writes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("script.sql");
    std::fs::write(
        &path,
        "SELECT 1;\nDELETE FROM t;\nUPDATE t SET a = 1 WHERE b = 2;\n",
    )?;
    let reads = scan_file(&path, POLICY, || false)?;
    assert_eq!(reads.statements, 3);
    // A DELETE without WHERE always asks; a filtered UPDATE runs.
    assert_eq!(reads.gate(), Gate::Confirm);
    assert_eq!(reads.samples.len(), 1);
    assert_eq!(reads.samples[0].line, 2);
    let confirm = SafetyPolicy {
        mode: openrows_types::safety::Safety::ConfirmWrites,
        ..POLICY
    };
    assert_eq!(scan_file(&path, confirm, || false)?.confirm, 2);
    let read_only = SafetyPolicy {
        mode: openrows_types::safety::Safety::ReadOnly,
        ..POLICY
    };
    let blocked = scan_file(&path, read_only, || false)?;
    assert_eq!(blocked.gate(), Gate::Block);
    assert_eq!(blocked.samples[0].verdict, Verdict::Block);
    let token = reads.confirmation_token("profile/db", &path);
    std::fs::write(&path, "SELECT 1;\nDELETE FROM t;\nDELETE FROM u;\n")?;
    let changed = scan_file(&path, POLICY, || false)?;
    assert_ne!(changed.confirmation_token("profile/db", &path), token);
    Ok(())
}

#[tokio::test]
async fn a_read_only_connection_refuses_writes_even_without_a_check() -> TestResult {
    let fixture = fixture().await?;
    let path = fixture.script("script.sql", SCRIPT.as_bytes())?;
    let run = FileRun {
        path: &path,
        policy: SafetyPolicy {
            mode: openrows_types::safety::Safety::ReadOnly,
            ..POLICY
        },
        stop_on_error: false,
        transaction: false,
        stamp: None,
    };
    let summary = run_file(
        &fixture.manager,
        fixture.session,
        run,
        &ScriptCanceller::default(),
        drop,
    )
    .await;
    assert_eq!(summary.outcome, FileOutcome::Failed);
    assert_eq!(summary.statements, 0);
    assert!(summary.message.unwrap_or_default().contains("read-only"));
    Ok(())
}
