// SPDX-License-Identifier: Apache-2.0
//! Statement and stored-result exports against the scripted driver.
use super::{FakeCancel, TestResult, manager, open};
use crate::export::{ExportFailure, ExportJob, ExportOutcome, export_batches, export_statement};
use crate::script::ScriptCanceller;
use openrows_export::{CsvOptions, Delimiter, ExportError, Exporter, Format, InsertOptions};
use openrows_results::{Append, Limits, ResultStore, SpillDir};
use openrows_sql::Dialect;
use openrows_sql::select::BoundSql;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

fn csv() -> Format {
    Format::Csv(CsvOptions {
        delimiter: Delimiter::Comma,
        header: true,
        ..CsvOptions::default()
    })
}

fn statement(sql: &str) -> BoundSql {
    BoundSql {
        sql: sql.to_owned(),
        params: Vec::new(),
    }
}

fn job(path: &Path, format: &Format, cancel: &ScriptCanceller) -> TestResult<ExportJob> {
    Ok(ExportJob {
        exporter: Exporter::create(path, format)?,
        cancel: cancel.clone(),
        progress: Box::new(|_| {}),
    })
}

fn files(directory: &Path) -> TestResult<usize> {
    Ok(std::fs::read_dir(directory)?.count())
}

#[tokio::test]
async fn a_statement_streams_into_the_file_and_not_the_store() -> TestResult {
    let (manager, store) = manager();
    let session = open(&manager, None).await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("rows.csv");
    let seen = Arc::new(AtomicU64::new(0));
    let reported = seen.clone();
    let job = ExportJob {
        progress: Box::new(move |rows| reported.store(rows, Ordering::SeqCst)),
        ..job(&path, &csv(), &ScriptCanceller::default())?
    };
    let outcome = export_statement(&manager, session, statement("rows 3 4"), job).await;
    assert!(
        matches!(outcome, ExportOutcome::Written { rows: 12 }),
        "{outcome:?}"
    );
    let text = std::fs::read_to_string(&path)?;
    assert_eq!(text.lines().count(), 13);
    assert!(text.starts_with("n\r\n0\r\n1\r\n"));
    assert!(
        seen.load(Ordering::SeqCst) >= 4,
        "the first batch is reported"
    );
    assert_eq!(
        store.batches(),
        0,
        "export rows never reach the result store"
    );
    assert_eq!(files(directory.path())?, 1);
    let mut next = manager.execute(session, "rows 1 1")?;
    assert!(super::last_event(&mut next).await?.is_terminal());
    assert_eq!(store.batches(), 1, "later queries use the store again");
    Ok(())
}

#[tokio::test]
async fn a_failing_statement_reports_the_server_error_and_leaves_no_file() -> TestResult {
    let (manager, _store) = manager();
    let session = open(&manager, None).await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fail.csv");
    let cancel = ScriptCanceller::default();
    let outcome = export_statement(
        &manager,
        session,
        statement("fail"),
        job(&path, &csv(), &cancel)?,
    )
    .await;
    let ExportOutcome::Failed(ExportFailure::Query(error)) = outcome else {
        return Err(format!("expected a query failure, got {outcome:?}").into());
    };
    assert_eq!(error.db_code.as_deref(), Some("42601"));
    assert_eq!(files(directory.path())?, 0);
    Ok(())
}

#[tokio::test]
async fn cancel_stops_the_statement_and_deletes_the_partial_file() -> TestResult {
    let (manager, _store) = manager();
    let driver_cancel = Arc::new(FakeCancel::default());
    let session = open(&manager, Some(driver_cancel.clone())).await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("hang.csv");
    let cancel = ScriptCanceller::default();
    let stopper = cancel.clone();
    let stop = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        stopper.cancel();
    });
    let outcome = export_statement(
        &manager,
        session,
        statement("hang"),
        job(&path, &csv(), &cancel)?,
    )
    .await;
    stop.await?;
    assert!(matches!(outcome, ExportOutcome::Cancelled), "{outcome:?}");
    assert_eq!(driver_cancel.requests.load(Ordering::SeqCst), 1);
    assert_eq!(files(directory.path())?, 0);
    let already = ScriptCanceller::default();
    already.cancel();
    let outcome = export_statement(
        &manager,
        session,
        statement("rows 1 1"),
        job(&path, &csv(), &already)?,
    )
    .await;
    assert!(matches!(outcome, ExportOutcome::Cancelled));
    assert_eq!(files(directory.path())?, 0);
    Ok(())
}

#[tokio::test]
async fn a_writer_failure_stops_the_stream_and_fails_the_export() -> TestResult {
    let (manager, _store) = manager();
    let session = open(&manager, None).await?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("bad.sql");
    let format = Format::Insert(InsertOptions {
        dialect: Dialect::Postgres,
        table: "bad\0name".to_owned(),
        rows_per_statement: NonZeroUsize::MIN,
    });
    let cancel = ScriptCanceller::default();
    let outcome = export_statement(
        &manager,
        session,
        statement("rows 50 10"),
        job(&path, &format, &cancel)?,
    )
    .await;
    assert!(
        matches!(
            outcome,
            ExportOutcome::Failed(ExportFailure::Write(ExportError::InvalidName(_)))
        ),
        "{outcome:?}"
    );
    assert_eq!(files(directory.path())?, 0);
    Ok(())
}

#[tokio::test]
async fn stored_batches_export_or_cancel() -> TestResult {
    let batch = openrows_results::synthetic::batch(5, 2, 7)?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("stored.json");
    let format = Format::Json(openrows_export::JsonLayout::Lines);
    let cancel = ScriptCanceller::default();
    let store = ResultStore::default();
    store.insert("r".to_owned(), vec![batch.clone(), batch])?;
    let stored = store.batches("r").ok_or("missing result")?;
    let outcome = export_batches(stored, job(&path, &format, &cancel)?).await;
    assert!(
        matches!(outcome, ExportOutcome::Written { rows: 10 }),
        "{outcome:?}"
    );
    assert_eq!(std::fs::read_to_string(&path)?.lines().count(), 10);
    let other = directory.path().join("cancelled.json");
    cancel.cancel();
    let stored = store.batches("r").ok_or("missing result")?;
    let outcome = export_batches(stored, job(&other, &format, &cancel)?).await;
    assert!(matches!(outcome, ExportOutcome::Cancelled));
    assert_eq!(files(directory.path())?, 1);
    Ok(())
}

#[tokio::test]
async fn spilled_results_export_every_row_in_order() -> TestResult {
    let spill = tempfile::tempdir()?;
    let batch = openrows_results::synthetic::batch(1_000, 4, 7)?;
    let limits = Limits {
        max_rows: usize::MAX,
        max_bytes: batch.get_array_memory_size(),
    };
    let store = ResultStore::with_spill(limits, SpillDir::create(spill.path())?)?;
    store.begin("r", batch.schema());
    for start in (0..1_000).step_by(100) {
        assert_eq!(store.append("r", batch.slice(start, 100)), Append::Continue);
    }
    store.settle();
    assert!(store.memory() <= limits.max_bytes);
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("spilled.csv");
    let cancel = ScriptCanceller::default();
    let stored = store.batches("r").ok_or("missing result")?;
    let outcome = export_batches(stored, job(&path, &csv(), &cancel)?).await;
    assert!(
        matches!(outcome, ExportOutcome::Written { rows: 1_000 }),
        "{outcome:?}"
    );
    let expected = directory.path().join("memory.csv");
    let in_memory = (
        batch.schema(),
        vec![Ok::<_, std::convert::Infallible>(batch)],
    );
    let outcome = export_batches(in_memory, job(&expected, &csv(), &cancel)?).await;
    assert!(matches!(outcome, ExportOutcome::Written { rows: 1_000 }));
    assert_eq!(std::fs::read(&path)?, std::fs::read(&expected)?);
    Ok(())
}
