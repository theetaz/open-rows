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
//! | [`Group::Lifecycle`] | `TableLifecycle` | A table created with `Driver::object_sql`, an index created and dropped with `Driver::alter_sql`, and a view created over the table are listed by the catalog after `commit_ddl` and gone after their drops; a view over a `DELETE` is refused before any SQL runs. |
//! | [`Group::Processes`] | `ProcessList` | The listing connection appears exactly once, marked current, with its own process id; a sleeping statement on a second connection is listed with its query text, and cancelling it from the first ends it within seconds with the dialect's cancel SQLSTATE while its session stays usable (SQL Server, which cannot cancel another session's statement, refuses with `Unsupported`); terminating the second connection ends it; signalling a process that is gone is a `Server` error `42704`. |
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
mod lifecycle;
mod metadata;
mod metadata_objects;
mod mongodb;
mod processes;
mod sink;
mod streaming;
mod structure;
mod types;

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

/// One group of the suite; see the crate documentation for each contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Group {
    /// Connect, ping, failure kinds and capability coherence.
    Connect,
    /// Typed round-trips of the common types, literal and bound.
    Types,
    /// Batching, ordering and early stop.
    Streaming,
    /// Out-of-band cancel.
    Cancel,
    /// SQLSTATE mapping of common server errors.
    Errors,
    /// Catalog listings.
    Metadata,
    /// Row keys, snapshots, guarded DML and transactional DDL.
    Edits,
    /// Generated structure changes.
    Structure,
    /// Generated create and drop of tables, indexes and views.
    Lifecycle,
    /// Process list, cancel and terminate from another connection.
    Processes,
}

impl Group {
    /// Every group, in run order.
    pub const ALL: [Self; 10] = [
        Self::Connect,
        Self::Types,
        Self::Streaming,
        Self::Cancel,
        Self::Errors,
        Self::Metadata,
        Self::Edits,
        Self::Structure,
        Self::Lifecycle,
        Self::Processes,
    ];

    /// The feature a driver must claim for the group to run.
    #[must_use]
    pub fn requires(self) -> Option<Feature> {
        match self {
            Self::Cancel => Some(Feature::Cancel),
            Self::Metadata => Some(Feature::Catalog),
            Self::Edits => Some(Feature::RowEdits),
            Self::Structure => Some(Feature::StructureChanges),
            Self::Lifecycle => Some(Feature::TableLifecycle),
            Self::Processes => Some(Feature::ProcessList),
            Self::Connect | Self::Types | Self::Streaming | Self::Errors => None,
        }
    }

    /// Run the group against `target`.
    ///
    /// # Errors
    /// The first broken contract or unexpected driver error.
    pub async fn run(self, target: &Target) -> Result<Verdict, Failure> {
        if let Some(feature) = self.requires()
            && !target.driver.capabilities().supports(feature)
        {
            return Ok(Verdict::Skipped(feature));
        }
        if target.driver.dialect() == openrows_sql::Dialect::MongoDb {
            mongodb::run(self, target).await?;
            return Ok(Verdict::Passed);
        }
        match self {
            Self::Connect => connect::run(target).await?,
            Self::Types => types::run(target).await?,
            Self::Streaming => streaming::run(target).await?,
            Self::Cancel => streaming::cancel(target).await?,
            Self::Errors if target.driver.dialect() == openrows_sql::Dialect::ClickHouse => {
                clickhouse::errors(target).await?;
            }
            Self::Errors => errors::run(target).await?,
            Self::Metadata if target.driver.dialect() == openrows_sql::Dialect::ClickHouse => {
                clickhouse::metadata(target).await?;
            }
            Self::Metadata => metadata::run(target).await?,
            Self::Edits => edits::run(target).await?,
            Self::Structure => structure::run(target).await?,
            Self::Lifecycle => lifecycle::run(target).await?,
            Self::Processes if target.driver.dialect() == openrows_sql::Dialect::ClickHouse => {
                clickhouse::processes(target).await?;
            }
            Self::Processes => processes::run(target).await?,
        }
        Ok(Verdict::Passed)
    }

    /// Lower-case name used in fixture object names.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Connect => "connect",
            Self::Types => "types",
            Self::Streaming => "streaming",
            Self::Cancel => "cancel",
            Self::Errors => "errors",
            Self::Metadata => "metadata",
            Self::Edits => "edits",
            Self::Structure => "structure",
            Self::Lifecycle => "lifecycle",
            Self::Processes => "processes",
        }
    }
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
    let verdict = runtime.block_on(group.run(&target));
    drop(guard);
    verdict
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
        assert!(fixture_name(Group::Edits, "t").starts_with("orc_edits_"));
        assert!(ensure(false, || "no".to_owned()).is_err());
    }
}
