// SPDX-License-Identifier: Apache-2.0
//! Engine layouts, instrumentation absence, protocol bounds and reset continuity.
use super::{
    Config, Probed, SqlText, bounded::Bounded, discovery, failure, layout::Layout, replication,
    status::Epoch,
};
use crate::wire::Rows;
use mysql_common::{constants::ColumnType, packets::Column};
use openrows_driver_api::DriverError;
use openrows_monitor::{Reading, SampleState, Source};
use std::collections::BTreeMap;

#[test]
fn distinct_version_matrix() {
    for (version, maria, process, locks) in [
        ("5.7.44", false, false, false),
        ("8.0.21", false, false, true),
        ("8.0.22", false, true, true),
        ("8.4.3", false, true, true),
        ("9.1.0", false, true, true),
        ("10.6.20-MariaDB", true, false, false),
        ("5.5.5-10.11.6-MariaDB", true, false, false),
        ("11.4.5-MariaDB", true, false, false),
    ] {
        assert_eq!(
            Layout::detect(version),
            Some(Layout {
                maria,
                modern_process: process,
                modern_locks: locks,
            })
        );
    }
    for version in [
        "",
        "bad",
        "10.5.9-MariaDB",
        "5.6.0",
        "10.0.0",
        "12.0.0-MariaDB",
    ] {
        assert_eq!(Layout::detect(version), None);
    }
}
fn config(layout: Layout, performance: bool) -> Config {
    Config {
        layout,
        performance,
        probed: Probed::default(),
        text: SqlText::Safe,
        descriptors: Vec::new(),
        epochs: BTreeMap::new(),
    }
}
fn layout(version: &str) -> Result<Layout, &'static str> {
    Layout::detect(version).ok_or("fixed test layout")
}
#[test]
fn performance_schema_disabled_is_not_an_empty_success() -> Result<(), &'static str> {
    for version in ["8.4.3", "11.4.5-MariaDB"] {
        let config = config(layout(version)?, false);
        for source in [
            Source::Statements,
            Source::Tables,
            Source::Indexes,
            Source::Io,
        ] {
            assert!(
                matches!(discovery::initial(source, &config, &Ok(Vec::new())), SampleState::Unavailable(reason) if reason.contains("performance_schema disabled") && reason.contains("restart"))
            );
        }
        for source in [Source::Activity, Source::Database, Source::Replication] {
            assert_eq!(
                discovery::initial(source, &config, &Ok(Vec::new())),
                SampleState::Ok
            );
        }
    }
    Ok(())
}
#[test]
fn instrument_and_consumer_matrix_gates_each_source() -> Result<(), &'static str> {
    let row = |values: [&str; 6]| values.map(|v| Some(v.to_owned())).to_vec();
    let all = row(["5", "3", "4", "3", "1", "2"]);
    for source in [
        Source::Statements,
        Source::Tables,
        Source::Indexes,
        Source::Io,
    ] {
        assert!(discovery::enabled(source, &all), "{source:?}");
    }
    let untimed_statements = row(["0", "3", "4", "3", "1", "2"]);
    assert!(!discovery::enabled(Source::Statements, &untimed_statements));
    let no_digest_consumer = row(["5", "3", "4", "2", "1", "2"]);
    assert!(!discovery::enabled(Source::Statements, &no_digest_consumer));
    let no_thread_consumers = row(["5", "3", "4", "3", "1", "1"]);
    assert!(!discovery::enabled(Source::Tables, &no_thread_consumers));
    assert!(!discovery::enabled(Source::Io, &no_thread_consumers));
    let no_file_instruments = row(["5", "3", "0", "3", "1", "2"]);
    assert!(!discovery::enabled(Source::Io, &no_file_instruments));
    assert!(discovery::enabled(Source::Indexes, &no_file_instruments));
    assert!(!discovery::enabled(
        Source::Statements,
        &[None, None, None, None, None, None]
    ));
    let denied = SampleState::PermissionDenied("x".into());
    let enabled = config(layout("8.4.3")?, true);
    assert_eq!(
        discovery::initial(Source::Statements, &enabled, &Err(denied.clone())),
        denied
    );
    Ok(())
}
#[test]
fn server_errors_name_engine_grants_or_become_gaps() -> Result<(), &'static str> {
    let (mysql, maria) = (layout("8.4.3")?, layout("11.4.5-MariaDB")?);
    let error = |code: u16, state: &str| crate::error::classify(code, state.into(), "x");
    let denied = |reading: Reading, grant: &str| matches!(reading.state, SampleState::PermissionDenied(reason) if reason.contains(grant));
    assert!(denied(
        failure(&error(1227, "42000"), Source::Activity, mysql),
        "PROCESS"
    ));
    assert!(denied(
        failure(&error(1227, "42000"), Source::Replication, mysql),
        "REPLICATION CLIENT"
    ));
    assert!(denied(
        failure(&error(1227, "42000"), Source::Replication, maria),
        "SLAVE MONITOR"
    ));
    assert!(denied(
        failure(&error(1142, "42000"), Source::Statements, maria),
        "SELECT"
    ));
    assert!(denied(
        failure(&error(1044, "42000"), Source::Indexes, mysql),
        "SELECT"
    ));
    for (code, state, layout) in [
        (3024, "HY000", mysql),
        (1969, "70100", maria),
        (1317, "70100", mysql),
    ] {
        assert_eq!(
            failure(&error(code, state), Source::Statements, layout).state,
            SampleState::Gap
        );
    }
    assert!(matches!(
        failure(&error(1146, "42S02"), Source::Indexes, mysql).state,
        SampleState::Unavailable(_)
    ));
    let lost = failure(
        &DriverError::Protocol("monitor result limit exceeded".into()),
        Source::Activity,
        mysql,
    );
    assert!(lost.terminal && lost.partial && lost.state == SampleState::Gap);
    Ok(())
}
#[test]
fn uptime_restart_and_flush_reset_epochs_without_tick_drift() {
    let mut epoch = Epoch::default();
    let first = epoch.observe(Some(100), Some(80));
    assert_eq!(first, epoch.observe(Some(101), Some(81)));
    let flushed = epoch.observe(Some(102), Some(0));
    assert_ne!(first, flushed);
    assert_ne!(flushed, epoch.observe(Some(1), Some(1)));
    assert_eq!(epoch.observe(None, None), "best effort");
}
#[test]
fn bounded_replica_rows_discard_addresses_and_null_stopped_lag() -> Result<(), DriverError> {
    let mut rows = Bounded::new(Some(replication::FIELDS));
    rows.begin(
        &[
            "Source_Host",
            "Replica_IO_Running",
            "Replica_SQL_Running",
            "Seconds_Behind_Source",
        ]
        .map(|n| Column::new(ColumnType::MYSQL_TYPE_VAR_STRING).with_name(n.as_bytes())),
    )?;
    rows.row(
        ["test-only", "Yes", "No", "0"]
            .map(|s| Some(s.as_bytes().to_vec()))
            .to_vec(),
    )?;
    assert!(!rows.names.iter().any(|n| n == "Source_Host"));
    let reading = replication::reading(&rows);
    assert_eq!(reading.rows[0][3], None);
    assert!(!format!("{reading:?}").contains("test-only"));
    Ok(())
}
#[test]
fn bounded_rows_reject_bytes_and_fail_closed_on_truncated_text() -> Result<(), DriverError> {
    let mut rows = Bounded::default();
    rows.begin(&[Column::new(ColumnType::MYSQL_TYPE_VAR_STRING).with_name(b"query")])?;
    rows.row(vec![Some(vec![b'x'; 4096])])?;
    assert_eq!(rows.rows[0][0], None);
    assert!(rows.row(vec![Some(vec![b'x'; 1_024 * 1_024])]).is_err());
    Ok(())
}

#[test]
fn maria_replica_lag_is_milliseconds_and_null_is_unknown() -> Result<(), DriverError> {
    let mut rows = Bounded::new(Some(replication::FIELDS));
    rows.begin(
        &[
            "Slave_IO_Running",
            "Slave_SQL_Running",
            "Seconds_Behind_Master",
        ]
        .map(|n| Column::new(ColumnType::MYSQL_TYPE_VAR_STRING).with_name(n.as_bytes())),
    )?;
    rows.row(
        ["Yes", "Yes", "3"]
            .map(|s| Some(s.as_bytes().to_vec()))
            .to_vec(),
    )?;
    rows.row(vec![Some(b"Yes".to_vec()), Some(b"Yes".to_vec()), None])?;
    let reading = replication::reading(&rows);
    assert_eq!(reading.rows[0][3].as_deref(), Some("3000"));
    assert_eq!(reading.rows[1][3], None);
    Ok(())
}
