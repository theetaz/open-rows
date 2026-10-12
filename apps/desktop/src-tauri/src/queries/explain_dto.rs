// SPDX-License-Identifier: Apache-2.0
//! Request, reply and plan types of `query_explain`.
use openrows_core::explain::Explained;
use openrows_explain::{AnalysedNode, Buffers, Detail, Format, Hotspot, Weight};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Explain one statement of the editor text.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExplainRequest {
    pub(crate) session_id: String,
    /// Client-chosen id; `query_cancel` with it cancels the explain.
    pub(crate) explain_id: String,
    /// The editor text, or only the selected text when `cursor` is `null`.
    pub(crate) sql: String,
    /// Cursor as a UTF-16 offset: explain the statement there; `null`
    /// explains `sql`, which must hold exactly one statement.
    pub(crate) cursor: Option<u32>,
    /// Run the statement and measure it (`EXPLAIN ANALYZE`).
    pub(crate) analyze: bool,
    /// Token from a `confirmationRequired` reply the user accepted.
    pub(crate) confirm_token: Option<String>,
}

/// How `query_explain` answered.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ExplainReply {
    /// The plan.
    Explained { plan: PlanDto },
    /// Nothing ran: measured execution or the safety policy requires consent; resend
    /// with `confirmToken` once the user accepted.
    ConfirmationRequired { token: String, statement: String },
    /// Nothing ran: read-only connection, or an engine that cannot roll back.
    Blocked { message: String },
    /// The user cancelled; any changes were rolled back.
    Cancelled,
}

/// What node shares measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PlanWeightDto {
    Time,
    Cost,
    None,
}

/// How the raw output is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PlanRawFormatDto {
    Json,
    Text,
}

/// A labelled value of a node or of the plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanDetailDto {
    label: String,
    value: String,
}

/// Block counts of a node (`PostgreSQL` `BUFFERS`), including its inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanBuffersDto {
    shared_hit: f64,
    shared_read: f64,
    shared_dirtied: f64,
    shared_written: f64,
    temp_read: f64,
    temp_written: f64,
}

/// Why a node deserves attention.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum PlanHotspotDto {
    Expensive { share: f64 },
    Misestimate { estimated: f64, actual: f64 },
    FullScan { rows: Option<f64> },
    Spill { reason: String },
}

/// One operator of the flattened plan (pre-order; parents first).
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanNodeDto {
    id: u32,
    parent: Option<u32>,
    depth: u32,
    node_type: String,
    relation: Option<String>,
    index: Option<String>,
    estimated_rows: Option<f64>,
    actual_rows: Option<f64>,
    loops: Option<f64>,
    startup_cost: Option<f64>,
    total_cost: Option<f64>,
    actual_startup_ms: Option<f64>,
    actual_total_ms: Option<f64>,
    inclusive_ms: Option<f64>,
    exclusive_ms: Option<f64>,
    inclusive_cost: Option<f64>,
    exclusive_cost: Option<f64>,
    /// Exclusive share of the plan's weight, 0–1.
    share: f64,
    buffers: Option<PlanBuffersDto>,
    conditions: Vec<PlanDetailDto>,
    details: Vec<PlanDetailDto>,
    hotspots: Vec<PlanHotspotDto>,
}

/// An analysed plan and the engine's own output.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanDto {
    /// The explained statement.
    statement: String,
    analyzed: bool,
    /// The statement ran and its changes were rolled back.
    rolled_back: bool,
    weight: PlanWeightDto,
    planning_ms: Option<f64>,
    execution_ms: Option<f64>,
    summary: Vec<PlanDetailDto>,
    nodes: Vec<PlanNodeDto>,
    raw: String,
    raw_format: PlanRawFormatDto,
}

impl PlanDto {
    /// Monitoring diagnostics strip predicates, raw output and text details before IPC.
    pub(crate) fn monitor(statement: &str, mut explained: Explained) -> Self {
        explained.raw =
            "Raw plan withheld for monitoring privacy; metric hotspots remain available.".into();
        explained.plan.summary.clear();
        explained.plan.nodes.truncate(256);
        for node in &mut explained.plan.nodes {
            node.data.conditions.clear();
            node.data.details.clear();
        }
        Self::new(
            openrows_sql::normalize_monitor_query(statement)
                .unwrap_or_else(|| "Statement text withheld".into()),
            explained,
        )
    }

    pub(crate) fn new(statement: String, explained: Explained) -> Self {
        let plan = explained.plan;
        Self {
            statement,
            analyzed: plan.analyzed,
            rolled_back: explained.rolled_back,
            weight: match plan.weight {
                Weight::Time => PlanWeightDto::Time,
                Weight::Cost => PlanWeightDto::Cost,
                Weight::None => PlanWeightDto::None,
            },
            planning_ms: plan.planning_ms,
            execution_ms: plan.execution_ms,
            summary: details(plan.summary),
            nodes: plan.nodes.into_iter().map(node).collect(),
            raw: explained.raw,
            raw_format: match explained.format {
                Format::PostgresJson | Format::MySqlJson => PlanRawFormatDto::Json,
                Format::MySqlTree | Format::SqliteRows | Format::SqlServerXml => {
                    PlanRawFormatDto::Text
                }
            },
        }
    }
}

fn index(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn details(details: Vec<Detail>) -> Vec<PlanDetailDto> {
    details
        .into_iter()
        .map(|detail| PlanDetailDto {
            label: detail.label,
            value: detail.value,
        })
        .collect()
}

fn buffers(buffers: Buffers) -> PlanBuffersDto {
    PlanBuffersDto {
        shared_hit: buffers.shared_hit,
        shared_read: buffers.shared_read,
        shared_dirtied: buffers.shared_dirtied,
        shared_written: buffers.shared_written,
        temp_read: buffers.temp_read,
        temp_written: buffers.temp_written,
    }
}

fn hotspot(hotspot: Hotspot) -> PlanHotspotDto {
    match hotspot {
        Hotspot::Expensive { share } => PlanHotspotDto::Expensive { share },
        Hotspot::Misestimate { estimated, actual } => {
            PlanHotspotDto::Misestimate { estimated, actual }
        }
        Hotspot::FullScan { rows } => PlanHotspotDto::FullScan { rows },
        Hotspot::Spill { reason } => PlanHotspotDto::Spill { reason },
    }
}

fn node(node: AnalysedNode) -> PlanNodeDto {
    let data = node.data;
    PlanNodeDto {
        id: index(node.id),
        parent: node.parent.map(index),
        depth: index(node.depth),
        node_type: data.node_type,
        relation: data.relation,
        index: data.index,
        estimated_rows: data.estimated_rows,
        actual_rows: data.actual_rows,
        loops: data.loops,
        startup_cost: data.startup_cost,
        total_cost: data.total_cost,
        actual_startup_ms: data.actual_startup_ms,
        actual_total_ms: data.actual_total_ms,
        inclusive_ms: node.inclusive_ms,
        exclusive_ms: node.exclusive_ms,
        inclusive_cost: node.inclusive_cost,
        exclusive_cost: node.exclusive_cost,
        share: node.share,
        buffers: data.buffers.map(buffers),
        conditions: details(data.conditions),
        details: details(data.details),
        hotspots: node.hotspots.into_iter().map(hotspot).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_plan_never_returns_sampled_literals_or_raw_conditions() {
        let raw = r#"[{"Plan":{"Node Type":"Seq Scan","Relation Name":"events","Plan Rows":1,"Filter":"(key = 'test-only')","Actual Rows":1,"Actual Loops":1}}]"#;
        let parsed = openrows_explain::analyse(
            openrows_explain::parse(Format::PostgresJson, &[vec![Some(raw.into())]])
                .expect("fixture plan"),
        );
        let safe = PlanDto::monitor(
            "SELECT * FROM events WHERE key = 'test-only'",
            Explained {
                plan: parsed,
                raw: raw.into(),
                format: Format::PostgresJson,
                rolled_back: true,
            },
        );
        let json = serde_json::to_string(&safe).expect("safe plan");
        assert!(!json.contains("test-only"));
        assert!(safe.nodes[0].conditions.is_empty());
        assert_eq!(safe.nodes[0].actual_rows, Some(1.0));
        assert!(safe.raw.contains("withheld"));
    }
}
