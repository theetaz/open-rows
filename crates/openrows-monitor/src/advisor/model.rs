// SPDX-License-Identifier: Apache-2.0
//! Evidence contracts deliberately contain no sampled literals or executable handles.
use serde::Serialize;

/// Rule set revision; thresholds are documented in ADR-0011.
pub const RULE_VERSION: u32 = 1;
/// Maximum age of relation/index evidence (two expensive-source cadences).
pub const CATALOG_MAX_AGE_MS: f64 = 120_000.0;
/// Maximum age of an explicitly obtained plan.
pub const PLAN_MAX_AGE_MS: f64 = 600_000.0;

/// A source's freshness and completeness, required before absence-based advice.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    /// Wall-clock sample time.
    pub collected_ms: f64,
    /// A permitted, complete source snapshot.
    pub complete: bool,
}
impl Coverage {
    /// Reject missing, future, partial and stale snapshots.
    #[must_use]
    pub fn fresh(&self, now: f64, maximum: f64) -> bool {
        self.complete
            && self.collected_ms.is_finite()
            && now >= self.collected_ms
            && now - self.collected_ms <= maximum
    }
}

/// Scoped table metadata and maintenance observations.
#[derive(Clone, Debug, Default)]
pub struct TableEvidence {
    /// `PostgreSQL` relation OID.
    pub id: String,
    /// Exact namespace identifier.
    pub schema: String,
    /// Exact relation identifier.
    pub name: String,
    /// Planner row estimate, not observed cardinality.
    pub estimated_rows: Option<f64>,
    /// Relation storage bytes.
    pub size_bytes: Option<f64>,
    /// Estimated live tuples.
    pub live_tuples: Option<f64>,
    /// Estimated dead tuples.
    pub dead_tuples: Option<f64>,
    /// Modifications since last manual/automatic analyze.
    pub modifications: Option<f64>,
    /// Age of latest manual/automatic analyze.
    pub analyze_age_ms: Option<f64>,
    /// Age of latest manual/automatic vacuum.
    pub vacuum_age_ms: Option<f64>,
    /// Consecutive valid observations in this reset epoch.
    pub samples: u32,
}

/// Index definitions are represented by catalog identities and a literal-free signature.
#[derive(Clone, Debug, Default)]
pub struct IndexEvidence {
    /// Index OID.
    pub id: String,
    /// Parent table OID.
    pub table_id: String,
    /// Exact namespace identifier.
    pub schema: String,
    /// Exact index identifier.
    pub name: String,
    /// Simple ordered key column names; expressions are excluded.
    pub keys: Vec<String>,
    /// Signature including keys, includes, predicate, expressions, opclasses and collations.
    pub signature: String,
    /// A valid, ready, non-partial ordinary btree index.
    pub usable: bool,
    /// PK, unique, constraint-supporting or replica-identity indexes cannot be dropped.
    pub protected: bool,
    /// Cumulative reads since statistics reset.
    pub scans: Option<f64>,
    /// Known database statistics-reset age; may overstate a new index's lifetime.
    pub reset_age_ms: Option<f64>,
}

/// A safe plan-node projection; predicates, raw plans and sampled values are excluded.
#[derive(Clone, Debug, Default)]
pub struct NodeEvidence {
    /// Visualiser node ID.
    pub id: u32,
    /// Exact catalog table OID, resolved without ambiguous names.
    pub table_id: Option<String>,
    /// `PostgreSQL` operator name.
    pub operator: String,
    /// Estimated output rows per loop.
    pub estimated_rows: Option<f64>,
    /// Actual output rows per loop, only in a measured plan.
    pub actual_rows: Option<f64>,
    /// Actual loops; zero means this node was not executed.
    pub loops: Option<f64>,
    /// A simple equality filter key, resolved from the plan condition.
    pub filter_column: Option<String>,
    /// Equality join keys belonging to this relation.
    pub join_columns: Vec<String>,
    /// Actual sort/hash spill reported by this node.
    pub spill: bool,
}

/// Evidence from an explicit diagnostic; estimates never become measured improvement.
#[derive(Clone, Debug, Default)]
pub struct PlanEvidence {
    /// Associated engine statement identity.
    pub statement_id: String,
    /// Explicit diagnostic completion time.
    pub collected_ms: f64,
    /// Whether execution was explicitly measured.
    pub measured: bool,
    /// Bounded literal-free node projections.
    pub nodes: Vec<NodeEvidence>,
}

/// One evaluated rule; SQL is a proposal string and cannot execute in this crate.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ipc", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    /// Stable object/query-scoped identity for local dismissal.
    pub id: String,
    /// Stable rule identifier.
    pub rule_id: String,
    /// Inspectable rule-set revision.
    pub version: u32,
    /// Low, medium or high evidence confidence.
    pub confidence: String,
    /// Qualitative expected impact, never promised speed-up.
    pub impact: String,
    /// Workload/locking/storage caveats.
    pub trade_offs: String,
    /// Exact numeric evidence and threshold explanation.
    pub evidence: String,
    /// Source table, statement identity or visualiser node link.
    pub evidence_link: String,
    /// Reviewed proposal, absent for application/memory hints.
    pub sql: Option<String>,
    /// Query scope, absent for connection-wide maintenance.
    pub statement_id: Option<String>,
    /// Source collection time.
    pub collected_ms: f64,
}

/// Current blocking edge observed across complete activity snapshots.
#[derive(Clone, Debug, Default)]
pub struct LockEvidence {
    /// Session and statement-start identity, not normalized SQL grouping.
    pub identity: String,
    /// Waiting backend PID.
    pub pid: String,
    /// Direct blocking backend PIDs.
    pub blockers: String,
    /// Current source wait event.
    pub wait: String,
    /// Current transaction age, not wait duration.
    pub transaction_ms: Option<f64>,
    /// Consecutive complete snapshots with this same blocking edge.
    pub samples: u32,
}

/// All rule inputs for a deterministic snapshot evaluation.
#[derive(Clone, Debug, Default)]
pub struct AdvisorInput {
    /// Evaluation clock supplied by the caller.
    pub now_ms: f64,
    /// Table coverage.
    pub tables_coverage: Coverage,
    /// Index coverage.
    pub indexes_coverage: Coverage,
    /// Complete activity coverage.
    pub activity_coverage: Coverage,
    /// Current repeated blocking edges.
    pub locks: Vec<LockEvidence>,
    /// Bounded table metadata.
    pub tables: Vec<TableEvidence>,
    /// Bounded index metadata.
    pub indexes: Vec<IndexEvidence>,
    /// Explicit diagnostics only.
    pub plans: Vec<PlanEvidence>,
    /// Valid statement intervals; no sampled text.
    pub intervals: Vec<crate::StatementInterval>,
}
