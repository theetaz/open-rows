// SPDX-License-Identifier: Apache-2.0
//! The spans of session open and query run log outcomes and counts, never the
//! SQL text, its literals or any row (docs/14 §6).
use super::{TestResult, manager, open};
use crate::script::{Script, ScriptCanceller, run_script};
use openrows_sql::select::BoundSql;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tracing::subscriber::with_default;

/// A cloneable in-memory log that the subscriber writes into.
#[derive(Clone, Default)]
struct Buffer(Arc<Mutex<Vec<u8>>>);

impl Buffer {
    fn text(&self) -> String {
        let bytes = self
            .0
            .lock()
            .map(|bytes| bytes.to_vec())
            .unwrap_or_default();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Write for Buffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(mut bytes) = self.0.lock() {
            bytes.extend_from_slice(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const LITERAL: &str = "LITERAL-7f3a";

#[test]
fn query_runs_log_their_outcome_but_never_the_sql_text() -> TestResult {
    let log = Buffer::default();
    let writer = log.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .finish();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let statements = vec![BoundSql {
        sql: format!("rows 1 1 SELECT '{LITERAL}' AS secret FROM accounts"),
        params: Vec::new(),
    }];
    with_default(subscriber, || {
        runtime.block_on(async {
            let (manager, _) = manager();
            let session = open(&manager, None).await?;
            let script = Script {
                statements: &statements,
                max_rows: None,
                stop_on_error: true,
            };
            let outcome = run_script(
                &manager,
                session,
                script,
                &ScriptCanceller::default(),
                |_| {},
            )
            .await;
            assert_eq!(outcome, crate::script::ScriptOutcome::Completed);
            TestResult::Ok(())
        })
    })?;
    let text = log.text();
    assert!(text.contains("session opened"), "{text}");
    assert!(text.contains("query run finished"), "{text}");
    assert!(!text.contains(LITERAL), "a literal reached the log: {text}");
    assert!(
        !text.contains("accounts"),
        "SQL text reached the log: {text}"
    );
    Ok(())
}
