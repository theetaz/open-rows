// SPDX-License-Identifier: Apache-2.0
//! Driver conformance suite (M1-10, docs/05 §8).
//!
//! Every driver must pass the groups below. A group runs only when the
//! driver's [`Capabilities`](openrows_driver_api::Capabilities) claim the
//! feature it needs; otherwise it reports [`Verdict::Skipped`]. The
//! [`Group::Connect`] group also checks that a live connection agrees with
//! the claimed capabilities, so a capability can never be claimed without
//! the behaviour behind it, nor the behaviour shipped without the claim.
//!
//! | Group | Needs | Contract |
//! | --- | --- | --- |
//! | [`Group::Connect`] | — | Connects and records the stages; `SELECT 1` answers; the server summary names the driver's product; facets, the cancel handle, `supports_alter` and the schema fingerprint match the capabilities; with `StartupCommands`, startup SQL runs on connect and a failing command fails the connect naming it; a wrong password is `Auth` (servers) and a missing file fails to open (files). |
//! | [`Group::Types`] | — (bound part: `BindParameters`) | NULL, integers, floats, booleans, unicode and empty text, binary, decimals and dates decode to the documented Arrow type and exact value; every field carries `openrows.source_type`; bound text and NULL round-trip unchanged. |
//! | [`Group::Streaming`] | — | 10 000 rows stream as one result in order, the first batch holds at most 200 rows, the summary counts every row; a sink that stops after the first batch ends the statement cleanly and the session stays usable. |
//! | [`Group::Cancel`] | `Cancel` | A long statement cancelled from its handle ends within seconds with the dialect's cancel SQLSTATE; the session stays usable. |
//! | [`Group::Errors`] | — | Syntax errors, unknown tables, unique and NOT NULL violations are `Server` errors with the dialect's SQLSTATE, and the session stays usable after each. |
//! | [`Group::Metadata`] | `Catalog` | The fixture database, schema, table and view are listed with the right kinds; the search index finds them; columns come back in order with types, nullability and the primary key. |
//! | [`Group::Structure`] | `StructureChanges` | A generated change set (type change, column rename, added column with a default, new index) commits through `commit_ddl` and the table reads back as staged with its row intact; an unknown column is refused before any SQL. |
//! | [`Group::Edits`] | `RowEdits` | The row key is discovered; snapshots are exact text; generated insert/update/delete commit atomically; a stale guard rolls the whole set back with `40001`; with `TransactionalDdl`, a failing DDL set leaves nothing behind. |
//! | [`Group::Lifecycle`] | `TableLifecycle` | A table created with `Driver::object_sql`, an index created and dropped with `Driver::alter_sql`, and a view created over the table are listed by the catalog after `commit_ddl` and gone after their drops; capability-gated key enforcement, USING conversion, concurrent indexes, column positions, triggers, routine definitions and database create/rename/drop are verified; a view over a `DELETE` is refused before any SQL runs. |
//! | [`Group::Processes`] | `ProcessList` | The listing connection appears exactly once, marked current, with its own process id; a sleeping statement on a second connection is listed with its query text, and cancelling it from the first ends it within seconds with the dialect's cancel SQLSTATE while its session stays usable (SQL Server, which cannot cancel another session's statement, refuses with `Unsupported`); terminating the second connection ends it; signalling a process that is gone is a `Server` error `42704`. |
//!
//! The Tier 1 engines also run the groups below, through
//! [`tier1_conformance_tests!`]:
//!
//! | Group | Needs | Contract |
//! | --- | --- | --- |
//! | [`Group::Transactions`] | `RowEdits` | An open transaction is reported and invisible to a second connection; commit publishes it, rollback discards it, a closed connection rolls it back; a failed statement aborts the transaction on `PostgreSQL` (`25P02` until it ends, `COMMIT` rolls back) and only the statement on `MySQL` and `SQLite`; a savepoint recovers from it on every engine; DDL commits implicitly exactly where `TransactionalDdl` is not claimed. |
//! | [`Group::Notices`] | `Notices` | A server message reaches the sink with its severity, in order, next to the rows of its statement and before a statement's error; the session stays usable. |
//! | [`Group::Introspection`] | `Catalog` | A fixture schema reads back through the catalog exactly as the checked-in golden file for the product says (`golden/`). |
//! | [`Group::MillionRows`] | — | 1 000 000 rows of 200-byte text stream with the exact count and sum, batches within the policy, and resident memory growing by far less than the result. |
//! | [`Group::Explain`] | `Explain` | The engine's plan and analysed plan parse into one operator tree naming the table; analysing a write leaves no change. |
//! | [`Group::Tls`] | `Tls` | Verified TLS reports the certificate and streams rows; a name the certificate does not cover and an unrelated CA are `Tls` errors at the TLS stage; `sslmode=disable` stays plain. |
//! | [`Group::Ssh`] | `Routes` | Through an OpenSSH tunnel a connection answers, streams, cancels and, with TLS files, verifies the server inside the tunnel. |
//!
//! A test crate states how to reach its engine and invokes
//! [`conformance_tests!`], which expands to one `#[test]` per group.
//! Fixture objects are named per process and group (`orc_<group>_<pid>`)
//! and dropped at the end, so test processes may share a server.

mod cell;
mod clickhouse;
mod connect;
mod dialect;
mod edits;
mod errors;
mod explain;
mod golden;
mod group;
mod lifecycle;
mod lifecycle_databases;
mod lifecycle_modules;
mod live;
mod metadata;
mod metadata_objects;
mod million;
mod mongodb;
mod notices;
mod processes;
mod routes;
mod sink;
mod streaming;
mod structure;
mod structure_options;
mod transactions;
mod types;

pub use group::Group;

use openrows_driver_api::{ConnectRequest, ConnectTrace, Connection, Driver, DriverError, Feature};

/// A driver and how to reach a test database with it.
pub struct Target {
    /// The driver under test.
    pub driver: Box<dyn Driver>,
    /// A request for a database where the suite may create and drop tables.
    pub request: ConnectRequest,
    /// The schema fixtures are created in (`public`, the `MySQL` database, `main`).
    pub schema: String,
}

impl Target {
    /// Open a fresh connection.
    ///
    /// # Errors
    /// The driver's connect error.
    pub async fn connect(&self) -> Result<Box<dyn Connection>, DriverError> {
        self.driver
            .connect(&self.request, &mut ConnectTrace::default())
            .await
    }
}

/// Why a group failed.
#[derive(Debug, thiserror::Error)]
pub enum Failure {
    /// The test environment is missing or unusable (e.g. no container).
    #[error("setup: {0}")]
    Setup(String),
    /// A driver call failed where the contract expects success.
    #[error(transparent)]
    Driver(#[from] DriverError),
    /// The driver behaved differently from the contract.
    #[error("contract violated: {0}")]
    Contract(String),
}

/// `Err(Failure::Contract)` with `message` unless `condition` holds.
pub(crate) fn ensure(condition: bool, message: impl FnOnce() -> String) -> Result<(), Failure> {
    if condition {
        Ok(())
    } else {
        Err(Failure::Contract(message()))
    }
}

/// How a group ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Verdict {
    /// Every check held.
    Passed,
    /// Not applicable: the driver does not claim this feature.
    Skipped(Feature),
}

/// A fixture object name unique to this process and group.
pub(crate) fn fixture_name(group: Group, suffix: &str) -> String {
    format!("orc_{}_{}_{suffix}", group.name(), std::process::id())
}

/// Build the target with `prepare` and run `group` on a current-thread
/// runtime. `prepare` also returns a guard (e.g. a temporary directory)
/// kept alive for the run.
///
/// # Errors
/// [`Failure::Setup`] when the runtime or the target cannot be prepared,
/// else the group's failure.
pub fn run_group<G>(
    group: Group,
    prepare: impl FnOnce() -> Result<(Target, G), Failure>,
) -> Result<Verdict, Failure> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| Failure::Setup(error.to_string()))?;
    let (target, guard) = prepare()?;
    let verdict = runtime.block_on(async {
        let target = live::refine(target).await?;
        group.run(&target).await
    });
    drop(guard);
    if let Ok(Verdict::Skipped(feature)) = verdict {
        note_skip(group, feature);
    }
    verdict
}

/// A gated group passes silently otherwise; visible with `--no-capture`.
#[allow(clippy::print_stderr, reason = "test diagnostics for the report")]
fn note_skip(group: Group, feature: Feature) {
    eprintln!("[conformance] {group:?} skipped: {feature:?} is not claimed");
}

/// One `#[test]` per [`Group`] for the target `prepare` builds; attributes
/// (e.g. `#[ignore = "needs Docker"]`) apply to every test.
///
/// ```ignore
/// openrows_driver_conformance::conformance_tests!(prepare);
/// ```
#[macro_export]
macro_rules! conformance_tests {
    ($(#[$attribute:meta])* $prepare:path) => {
        $crate::conformance_tests!(@tests [$(#[$attribute])*] $prepare;
            connect_and_ping => Connect,
            typed_round_trips => Types,
            streaming_contract => Streaming,
            cancel_contract => Cancel,
            error_sqlstates => Errors,
            metadata_listing => Metadata,
            edit_contract => Edits,
            structure_contract => Structure,
            lifecycle_contract => Lifecycle,
            process_list_contract => Processes,
        );
    };
    (@tests $attributes:tt $prepare:path; $($name:ident => $group:ident,)*) => {
        $($crate::conformance_tests!(@test $attributes $prepare; $name => $group);)*
    };
    (@test [$($attribute:tt)*] $prepare:path; $name:ident => $group:ident) => {
        #[test]
        $($attribute)*
        fn $name() -> Result<(), $crate::Failure> {
            $crate::run_group($crate::Group::$group, $prepare).map(drop)
        }
    };
}

/// One `#[test]` per [`Group::TIER1`] group, on top of
/// [`conformance_tests!`]. The TLS and SSH groups read their fixtures from
/// the environment `just test-pg` and `just test-mysql` export.
///
/// ```ignore
/// openrows_driver_conformance::tier1_conformance_tests!(prepare);
/// ```
#[macro_export]
macro_rules! tier1_conformance_tests {
    ($(#[$attribute:meta])* $prepare:path) => {
        $crate::conformance_tests!(@tests [$(#[$attribute])*] $prepare;
            transaction_contract => Transactions,
            notice_contract => Notices,
            golden_introspection => Introspection,
            million_row_stream => MillionRows,
            explain_contract => Explain,
            tls_route => Tls,
            ssh_route => Ssh,
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gated_groups_name_their_feature() {
        let gated: Vec<_> = Group::ALL.into_iter().filter_map(Group::requires).collect();
        assert_eq!(
            gated,
            [
                Feature::Cancel,
                Feature::Catalog,
                Feature::RowEdits,
                Feature::StructureChanges,
                Feature::TableLifecycle,
                Feature::ProcessList
            ]
        );
        let tier1: Vec<_> = Group::TIER1
            .into_iter()
            .filter_map(Group::requires)
            .collect();
        assert_eq!(
            tier1,
            [
                Feature::RowEdits,
                Feature::Notices,
                Feature::Catalog,
                Feature::Explain,
                Feature::Tls,
                Feature::Routes
            ]
        );
        assert!(fixture_name(Group::Edits, "t").starts_with("orc_edits_"));
        assert!(ensure(false, || "no".to_owned()).is_err());
    }
}
