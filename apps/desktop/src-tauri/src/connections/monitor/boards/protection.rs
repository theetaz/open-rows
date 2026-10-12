// SPDX-License-Identifier: Apache-2.0
//! Per-engine read-only protection of a widget's disposable connection.
use openrows_store::profile::Driver;
use serde::Serialize;
use specta::Type;

use crate::connections::{IpcError, IpcErrorKind};

/// How strongly a widget run is kept read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ReadOnlyMode {
    /// The engine refuses writes (read-only transaction or query-only session).
    Enforced,
    /// Statement classification plus a rolled-back transaction where the
    /// engine has one; manual runs only, no interval refresh.
    Classified,
}

/// Setup statements, whether a transaction is rolled back, and the mode.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Protection {
    pub(super) setup: Vec<String>,
    pub(super) rollback: bool,
    pub(super) mode: ReadOnlyMode,
}

/// The protection of `driver` with a statement deadline of `timeout_ms`.
pub(super) fn protection(driver: Driver, timeout_ms: u32) -> Result<Protection, IpcError> {
    let enforced = |setup: Vec<String>| Protection {
        setup,
        rollback: true,
        mode: ReadOnlyMode::Enforced,
    };
    Ok(match driver {
        Driver::Postgres => enforced(vec![
            "BEGIN READ ONLY".into(),
            format!("SET LOCAL statement_timeout = {timeout_ms}"),
            "SET LOCAL lock_timeout = 1000".into(),
        ]),
        Driver::Mysql => enforced(vec![
            format!("SET SESSION MAX_EXECUTION_TIME={timeout_ms}"),
            "START TRANSACTION READ ONLY".into(),
        ]),
        Driver::Mariadb => enforced(vec![
            format!(
                "SET SESSION max_statement_time={}",
                timeout_ms.div_ceil(1_000)
            ),
            "START TRANSACTION READ ONLY".into(),
        ]),
        Driver::Sqlite | Driver::LibsqlFile => {
            enforced(vec!["PRAGMA query_only=ON".into(), "BEGIN".into()])
        }
        Driver::Sqlserver => Protection {
            setup: vec!["SET LOCK_TIMEOUT 1000".into(), "BEGIN TRANSACTION".into()],
            rollback: true,
            mode: ReadOnlyMode::Classified,
        },
        Driver::Clickhouse | Driver::Libsql => Protection {
            setup: Vec::new(),
            rollback: false,
            mode: ReadOnlyMode::Classified,
        },
        Driver::Duckdb => {
            return Err(IpcError::new(
                IpcErrorKind::Unsupported,
                "DuckDB boards need their owning session; no extra connection is opened",
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engines_with_read_only_transactions_are_enforced_and_others_are_classified() {
        for driver in [
            Driver::Postgres,
            Driver::Mysql,
            Driver::Mariadb,
            Driver::Sqlite,
            Driver::LibsqlFile,
        ] {
            let plan = protection(driver, 15_000).ok();
            assert_eq!(plan.as_ref().map(|p| p.mode), Some(ReadOnlyMode::Enforced));
            let setup = plan.map(|p| p.setup.join(";")).unwrap_or_default();
            assert!(
                setup.contains("READ ONLY") || setup.contains("query_only=ON"),
                "{setup}"
            );
        }
        for driver in [Driver::Sqlserver, Driver::Clickhouse, Driver::Libsql] {
            let mode = protection(driver, 15_000).map(|p| p.mode).ok();
            assert_eq!(mode, Some(ReadOnlyMode::Classified));
        }
        assert!(protection(Driver::Duckdb, 15_000).is_err());
        let postgres = protection(Driver::Postgres, 2_500).map(|p| p.setup).ok();
        assert_eq!(
            postgres.and_then(|setup| setup.get(1).cloned()).as_deref(),
            Some("SET LOCAL statement_timeout = 2500")
        );
    }
}
