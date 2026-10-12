// SPDX-License-Identifier: Apache-2.0
//! Platform matrix, explicit permission states, error classes and bounds.
use std::borrow::Cow;
use std::sync::Arc;

use openrows_driver_api::DriverError;
use openrows_monitor::{SampleState, Source};

use super::platform::{Edition, Platform};
use super::{assemble, counters, discovery, failure, guard::Bounded, sources};
use crate::reply::Rows;
use crate::tds::token::Column;
use crate::tds::types::{Layout, SqlType, TypeInfo};

fn platform(version: &str, flavour: Option<&str>) -> Result<Platform, &'static str> {
    Platform::detect(version, flavour).ok_or("supported platform")
}

fn server(number: u32) -> DriverError {
    DriverError::Server {
        code: "HY000".into(),
        message: format!("text (SQL Server error {number}, line 1)"),
    }
}

#[test]
fn version_and_platform_matrix() -> Result<(), &'static str> {
    for (version, flavour, edition) in [
        ("14.0.3456.2", None, Edition::OnPremises),
        ("16.0.4135.4", None, Edition::OnPremises),
        ("15.0.2000.1574", Some("Azure SQL Edge"), Edition::Edge),
        (
            "12.0.2000.8",
            Some("Azure SQL Database"),
            Edition::AzureDatabase,
        ),
        (
            "12.0.2000.8",
            Some("Azure SQL Managed Instance"),
            Edition::ManagedInstance,
        ),
    ] {
        assert_eq!(platform(version, flavour)?.edition, edition);
    }
    assert!(Platform::detect("13.0.6300.2", None).is_none());
    assert!(Platform::detect("", None).is_none());
    assert!(Platform::detect("12.0.2000.8", Some("Microsoft Fabric")).is_none());
    let azure = platform("12.0.2000.8", Some("Azure SQL Database"))?;
    assert!(
        azure
            .permission_probe()
            .contains("VIEW DATABASE PERFORMANCE STATE")
    );
    assert!(
        sources::query(Source::Io, azure)
            .is_some_and(|(sql, _)| sql.contains("sys.dm_db_wait_stats")
                && sql.contains("@epoch varchar(30) = NULL"))
    );
    let old = platform("15.0.4000.1", None)?;
    assert!(!old.permission_probe().contains("PERFORMANCE STATE"));
    assert!(
        platform("16.0.1000.6", None)?
            .permission_probe()
            .contains("VIEW SERVER PERFORMANCE STATE")
    );
    Ok(())
}

#[test]
fn restricted_visibility_is_explicit_never_zero() -> Result<(), &'static str> {
    let platform = platform("16.0.4135.4", None)?;
    for source in Source::ALL {
        let state = discovery::initial(source, platform, false);
        match source {
            Source::Replication => assert!(matches!(state, SampleState::Unavailable(_))),
            _ => assert!(
                matches!(&state, SampleState::PermissionDenied(reason) if reason.contains("VIEW SERVER PERFORMANCE STATE")),
                "{source:?}"
            ),
        }
        if source != Source::Replication {
            assert_eq!(discovery::initial(source, platform, true), SampleState::Ok);
        }
    }
    Ok(())
}

#[test]
fn server_errors_map_to_explicit_states() -> Result<(), &'static str> {
    let platform = platform("15.0.2000.1574", Some("Azure SQL Edge"))?;
    for number in [229, 300, 297] {
        let reading = failure(&server(number), platform);
        assert!(matches!(reading.state, SampleState::PermissionDenied(_)));
        assert!(!reading.terminal && reading.rows.is_empty());
    }
    assert!(
        matches!(failure(&server(208), platform).state, SampleState::Unavailable(reason) if reason.contains("version/platform"))
    );
    assert_eq!(failure(&server(1222), platform).state, SampleState::Gap);
    let cancelled = DriverError::Server {
        code: "HY008".into(),
        message: "The statement was cancelled.".into(),
    };
    let reading = failure(&cancelled, platform);
    assert!(reading.state == SampleState::Gap && !reading.terminal);
    let lost = failure(
        &DriverError::Protocol("monitor result limit exceeded".into()),
        platform,
    );
    assert!(lost.state == SampleState::Gap && lost.terminal && lost.partial);
    Ok(())
}

fn text_columns(count: usize) -> Arc<[Column]> {
    (0..count)
        .map(|i| Column {
            name: format!("c{i}"),
            info: TypeInfo {
                sql: SqlType::Char {
                    collation: crate::tds::collation::Collation::default(),
                    varying: true,
                },
                layout: Layout::UShortLen,
            },
            user_type: 0,
        })
        .collect()
}

#[test]
fn bounded_rows_cap_cells_rows_and_columns() -> Result<(), DriverError> {
    let mut rows = Bounded::default();
    assert!(rows.begin(&text_columns(129)).is_err());
    rows.begin(&text_columns(2))?;
    let wide = vec![b'x'; 4_096];
    rows.row(vec![
        Some(Cow::Borrowed(&b"ok"[..])),
        Some(Cow::Owned(wide)),
    ])?;
    assert_eq!(rows.rows[0], vec![Some("ok".to_owned()), None]);
    assert!(rows.partial);
    for _ in 1..1_000 {
        rows.row(vec![None, None])?;
    }
    assert!(rows.row(vec![None, None]).is_err());
    Ok(())
}

#[test]
fn epoch_is_stripped_and_counters_stay_unknown() {
    let mut result = Bounded::default();
    result.rows = vec![vec![
        Some("server".into()),
        None,
        Some("2026-10-12T10:00:00".into()),
    ]];
    let reading = assemble(
        Source::Database,
        result,
        sources::columns("server_id:text,batch_requests:counter"),
    );
    assert_eq!(reading.epoch, "2026-10-12T10:00:00");
    assert_eq!(reading.rows, vec![vec![Some("server".to_owned()), None]]);
    let empty = assemble(Source::Io, Bounded::default(), Vec::new());
    assert_eq!(empty.epoch, "best effort");
    let columns = sources::columns(&counters::spec());
    assert_eq!(columns.len(), 14);
    let sql = counters::query();
    assert_eq!(sql.matches("MAX(CASE").count(), 11 + 2 * 2);
    assert!(sql.contains(
        "@epoch FROM sys.dm_os_performance_counters WHERE counter_name IN (N'Batch Requests/sec'"
    ));
    assert!(sql.ends_with("N'CPU usage %', N'CPU usage % base')"));
}
