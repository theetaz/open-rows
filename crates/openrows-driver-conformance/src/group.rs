// SPDX-License-Identifier: Apache-2.0
//! The groups of the suite: what each needs and how it runs.

use std::future::Future;
use std::pin::Pin;

use openrows_driver_api::Feature;
use openrows_sql::Dialect;

use crate::{Failure, Target, Verdict};

/// One group's contract, running on the test's current-thread runtime.
type Contract<'a> = Pin<Box<dyn Future<Output = Result<(), Failure>> + 'a>>;

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
    /// Visibility and failure behaviour of typed transactions.
    Transactions,
    /// Server messages with their severity.
    Notices,
    /// Catalog output against checked-in golden data.
    Introspection,
    /// One million rows with bounded memory.
    MillionRows,
    /// Plans and analysed plans in the engine's own format.
    Explain,
    /// Verified TLS and its refusals.
    Tls,
    /// A connection through an SSH tunnel.
    Ssh,
}

impl Group {
    /// The groups every driver runs ([`crate::conformance_tests!`]), in run order.
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

    /// The groups only Tier 1 engines run so far ([`crate::tier1_conformance_tests!`]).
    pub const TIER1: [Self; 7] = [
        Self::Transactions,
        Self::Notices,
        Self::Introspection,
        Self::MillionRows,
        Self::Explain,
        Self::Tls,
        Self::Ssh,
    ];

    /// The feature a driver must claim for the group to run.
    #[must_use]
    pub fn requires(self) -> Option<Feature> {
        match self {
            Self::Cancel => Some(Feature::Cancel),
            Self::Metadata | Self::Introspection => Some(Feature::Catalog),
            Self::Edits | Self::Transactions => Some(Feature::RowEdits),
            Self::Structure => Some(Feature::StructureChanges),
            Self::Lifecycle => Some(Feature::TableLifecycle),
            Self::Processes => Some(Feature::ProcessList),
            Self::Notices => Some(Feature::Notices),
            Self::Explain => Some(Feature::Explain),
            Self::Tls => Some(Feature::Tls),
            Self::Ssh => Some(Feature::Routes),
            Self::Connect | Self::Types | Self::Streaming | Self::Errors | Self::MillionRows => {
                None
            }
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
        if target.driver.dialect() == Dialect::MongoDb {
            crate::mongodb::run(self, target).await?;
        } else {
            self.contract(target).await?;
        }
        Ok(Verdict::Passed)
    }

    /// The group's contract for an engine with the SQL session model.
    fn contract(self, target: &Target) -> Contract<'_> {
        let clickhouse = target.driver.dialect() == Dialect::ClickHouse;
        match self {
            Self::Connect => Box::pin(crate::connect::run(target)),
            Self::Types => Box::pin(crate::types::run(target)),
            Self::Streaming => Box::pin(crate::streaming::run(target)),
            Self::Cancel => Box::pin(crate::streaming::cancel(target)),
            Self::Errors if clickhouse => Box::pin(crate::clickhouse::errors(target)),
            Self::Errors => Box::pin(crate::errors::run(target)),
            Self::Metadata if clickhouse => Box::pin(crate::clickhouse::metadata(target)),
            Self::Metadata => Box::pin(crate::metadata::run(target)),
            Self::Edits => Box::pin(crate::edits::run(target)),
            Self::Structure => Box::pin(crate::structure::run(target)),
            Self::Lifecycle => Box::pin(crate::lifecycle::run(target)),
            Self::Processes if clickhouse => Box::pin(crate::clickhouse::processes(target)),
            Self::Processes => Box::pin(crate::processes::run(target)),
            Self::Transactions => Box::pin(crate::transactions::run(target)),
            Self::Notices => Box::pin(crate::notices::run(target)),
            Self::Introspection => Box::pin(crate::golden::run(target)),
            Self::MillionRows => Box::pin(crate::million::run(target)),
            Self::Explain => Box::pin(crate::explain::run(target)),
            Self::Tls => Box::pin(crate::routes::tls(target)),
            Self::Ssh => Box::pin(crate::routes::ssh(target)),
        }
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
            Self::Transactions => "transactions",
            Self::Notices => "notices",
            Self::Introspection => "golden",
            Self::MillionRows => "million",
            Self::Explain => "explain",
            Self::Tls => "tls",
            Self::Ssh => "ssh",
        }
    }
}
