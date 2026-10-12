// SPDX-License-Identifier: Apache-2.0
//! The own-session ledger groups by literal-free shape, keeps engine counters
//! unknown when a transport has none, and resets with a new epoch.
use openrows_monitor::{EngineSteps, Execution, Ledger, LiveStore, Source};

fn run(ms: f64, rows: u64, steps: Option<EngineSteps>) -> Execution {
    Execution {
        elapsed_ms: ms,
        rows,
        failed: false,
        steps,
    }
}

fn cell<'a>(reading: &'a openrows_monitor::Reading, row: usize, name: &str) -> Option<&'a str> {
    let index = reading.columns.iter().position(|c| c.name == name)?;
    reading.rows.get(row)?.get(index)?.as_deref()
}

#[test]
fn statements_group_by_shape_rank_by_total_time_and_never_keep_literals() {
    let ledger = Ledger::shared("test:shapes");
    let steps = EngineSteps {
        fullscan_steps: 9,
        sorts: 1,
        vm_steps: 40,
    };
    ledger.record("SELECT * FROM t WHERE id = 1", run(2.0, 1, Some(steps)));
    ledger.record(
        "SELECT * FROM t WHERE id = 'secret'",
        run(3.0, 1, Some(steps)),
    );
    ledger.record("PRAGMA user_version = 7", run(50.0, 0, None));
    ledger.record(
        "UPDATE t SET a = 2",
        Execution {
            failed: true,
            ..run(1.0, 0, None)
        },
    );
    let reading = ledger.statements();
    assert_eq!(reading.rows.len(), 3);
    assert_eq!(cell(&reading, 0, "statement_id"), Some("other:PRAGMA"));
    assert_eq!(cell(&reading, 0, "query"), None);
    assert_eq!(cell(&reading, 0, "fullscan_steps"), None);
    assert_eq!(cell(&reading, 1, "calls"), Some("2"));
    assert_eq!(cell(&reading, 1, "total_ms"), Some("5.000"));
    assert_eq!(cell(&reading, 1, "max_since_reset_ms"), Some("3.000"));
    assert_eq!(cell(&reading, 1, "fullscan_steps"), Some("18"));
    let text = cell(&reading, 1, "query").unwrap_or_default();
    assert!(!text.contains("secret") && !text.contains('1'), "{text}");
    assert_eq!(cell(&reading, 2, "errors"), Some("1"));
    let totals = ledger.totals();
    assert_eq!((totals.requests, totals.errors, totals.rows), (4, 1, 2));
}

#[test]
fn shared_ledger_lives_while_held_and_restarts_with_a_new_epoch() {
    let first = Ledger::shared("test:lifetime");
    let again = Ledger::shared("test:lifetime");
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    let epoch = first.epoch().to_owned();
    first.record("SELECT 1", run(1.0, 1, None));
    drop((first, again));
    let fresh = Ledger::shared("test:lifetime");
    assert_ne!(fresh.epoch(), epoch);
    assert_eq!(fresh.totals().requests, 0);
}

#[test]
fn eviction_keeps_the_costliest_shapes_and_marks_the_reading_partial() {
    let ledger = Ledger::shared("test:evict");
    for table in 0..201 {
        ledger.record(
            &format!("SELECT a FROM t{table}"),
            run(f64::from(table) + 1.0, 0, None),
        );
    }
    let reading = ledger.statements();
    assert_eq!(reading.rows.len(), 200);
    assert!(reading.partial);
    assert!(
        reading
            .rows
            .iter()
            .all(|row| row.last().cloned().flatten().as_deref() != Some("SELECT a FROM t0"))
    );
}

#[test]
fn own_statements_become_interval_rates_in_the_live_store() -> Result<(), Box<dyn std::error::Error>>
{
    let ledger = Ledger::shared("test:store");
    let mut store = LiveStore::default();
    ledger.record("SELECT 1", run(4.0, 1, None));
    store.ingest(Source::Statements, ledger.statements(), 0, 0.0);
    ledger.record("SELECT 2", run(6.0, 1, None));
    store.ingest(Source::Statements, ledger.statements(), 1_000, 1_000.0);
    let window = store.window(Some(Source::Statements), 100, false)?;
    assert!(!window.is_empty());
    Ok(())
}
