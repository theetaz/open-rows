// SPDX-License-Identifier: Apache-2.0
//! Exclusive time and cost per operator, and the hotspots worth a look:
//! the most expensive operators, row misestimates, full scans of large
//! tables and sorts or hashes that spilled to disk.
use crate::model::{Detail, NodeData, Plan, PlanNode};

/// Actual and estimated rows differing by more than this factor are flagged.
pub const MISESTIMATE_RATIO: f64 = 10.0;
/// Full scans reading at least this many rows are flagged.
pub const LARGE_TABLE_ROWS: f64 = 10_000.0;
/// Operators with at least this share of the plan may be flagged expensive…
pub const EXPENSIVE_SHARE: f64 = 0.1;
/// …but only the top this many of them.
pub const EXPENSIVE_TOP: usize = 3;

/// Why an operator deserves attention.
#[derive(Clone, Debug, PartialEq)]
pub enum Hotspot {
    /// Among the operators taking most of the plan's time (or cost).
    Expensive {
        /// The operator's exclusive share of the plan, 0–1.
        share: f64,
    },
    /// Actual rows differ from the estimate by more than [`MISESTIMATE_RATIO`].
    Misestimate {
        /// Planner's estimate per loop.
        estimated: f64,
        /// Rows returned per loop.
        actual: f64,
    },
    /// Reads a whole table of at least [`LARGE_TABLE_ROWS`] rows (or of
    /// unknown size).
    FullScan {
        /// Rows read, when known.
        rows: Option<f64>,
    },
    /// A sort or hash went to disk.
    Spill {
        /// The engine's evidence, e.g. `Sort spilled to disk (external merge, 1832 kB)`.
        reason: String,
    },
}

/// What operator shares are measured in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Weight {
    /// Exclusive actual time (analysed plans).
    Time,
    /// Exclusive planner cost.
    Cost,
    /// Neither was reported (`SQLite` query plans).
    None,
}

/// One operator of the flattened plan with its derived measures.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysedNode {
    /// Position in pre-order; the root is 0.
    pub id: usize,
    /// The parent's id; `None` for the root.
    pub parent: Option<usize>,
    /// Distance from the root.
    pub depth: usize,
    /// What the engine reported.
    pub data: NodeData,
    /// Time of all loops including the inputs, in milliseconds.
    pub inclusive_ms: Option<f64>,
    /// Time spent in this operator alone, in milliseconds.
    pub exclusive_ms: Option<f64>,
    /// Cost including the inputs.
    pub inclusive_cost: Option<f64>,
    /// Cost of this operator alone.
    pub exclusive_cost: Option<f64>,
    /// Exclusive share of the plan's [`Weight`], 0–1.
    pub share: f64,
    /// Reasons to look at this operator.
    pub hotspots: Vec<Hotspot>,
}

/// A plan flattened in pre-order with measures and hotspots.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysedPlan {
    /// Operators in pre-order (parents before children).
    pub nodes: Vec<AnalysedNode>,
    /// What `share` measures.
    pub weight: Weight,
    /// The statement ran and actuals are present.
    pub analyzed: bool,
    /// Planning time in milliseconds, when reported.
    pub planning_ms: Option<f64>,
    /// Execution time in milliseconds, when reported.
    pub execution_ms: Option<f64>,
    /// Plan-wide values.
    pub summary: Vec<Detail>,
}

/// Inclusive cost: the node's own total, else the sum over its inputs.
pub(crate) fn inclusive_cost(node: &PlanNode) -> Option<f64> {
    node.data
        .total_cost
        .or_else(|| sum(node.children.iter().map(inclusive_cost)))
}

/// Inclusive time over all loops, else the sum over its inputs.
fn inclusive_ms(node: &PlanNode) -> Option<f64> {
    node.data
        .actual_total_ms
        .map(|per_loop| per_loop * node.data.loops.unwrap_or(1.0))
        .or_else(|| sum(node.children.iter().map(inclusive_ms)))
}

fn sum(values: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    values.flatten().reduce(|total, value| total + value)
}

/// What remains of `inclusive` after the inputs' share, never negative.
fn exclusive(inclusive: Option<f64>, inputs: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    inclusive.map(|total| (total - sum(inputs).unwrap_or(0.0)).max(0.0))
}

/// Flatten, measure and flag `plan`.
#[must_use]
pub fn analyse(plan: Plan) -> AnalysedPlan {
    let mut nodes = Vec::new();
    flatten(plan.root, None, 0, &mut nodes);
    let weight = weight_of(&nodes, plan.analyzed);
    shares(&mut nodes, weight);
    for node in &mut nodes {
        node.hotspots
            .extend(local_hotspots(&node.data, plan.analyzed));
    }
    mark_expensive(&mut nodes);
    AnalysedPlan {
        nodes,
        weight,
        analyzed: plan.analyzed,
        planning_ms: plan.planning_ms,
        execution_ms: plan.execution_ms,
        summary: plan.summary,
    }
}

fn flatten(node: PlanNode, parent: Option<usize>, depth: usize, out: &mut Vec<AnalysedNode>) {
    let id = out.len();
    let inclusive_ms = inclusive_ms(&node);
    let inclusive_cost = inclusive_cost(&node);
    let exclusive_ms = exclusive(inclusive_ms, node.children.iter().map(self::inclusive_ms));
    let exclusive_cost = exclusive(
        inclusive_cost,
        node.children.iter().map(self::inclusive_cost),
    );
    out.push(AnalysedNode {
        id,
        parent,
        depth,
        data: node.data,
        inclusive_ms,
        exclusive_ms,
        inclusive_cost,
        exclusive_cost,
        share: 0.0,
        hotspots: Vec::new(),
    });
    for child in node.children {
        flatten(child, Some(id), depth + 1, out);
    }
}

fn weight_of(nodes: &[AnalysedNode], analyzed: bool) -> Weight {
    if analyzed && nodes.iter().any(|node| node.exclusive_ms.is_some()) {
        Weight::Time
    } else if nodes.iter().any(|node| node.exclusive_cost.is_some()) {
        Weight::Cost
    } else {
        Weight::None
    }
}

fn measure(node: &AnalysedNode, weight: Weight) -> f64 {
    match weight {
        Weight::Time => node.exclusive_ms,
        Weight::Cost => node.exclusive_cost,
        Weight::None => None,
    }
    .unwrap_or(0.0)
}

fn shares(nodes: &mut [AnalysedNode], weight: Weight) {
    let total: f64 = nodes.iter().map(|node| measure(node, weight)).sum();
    if total <= 0.0 {
        return;
    }
    for node in nodes {
        node.share = measure(node, weight) / total;
    }
}

/// Hotspots judged on one operator alone.
fn local_hotspots(data: &NodeData, analyzed: bool) -> Vec<Hotspot> {
    let mut hotspots = Vec::new();
    if let (true, Some(estimated), Some(actual)) = (analyzed, data.estimated_rows, data.actual_rows)
        && data.loops != Some(0.0)
    {
        let (high, low) = (estimated.max(actual), estimated.min(actual));
        if high.max(1.0) / low.max(1.0) > MISESTIMATE_RATIO {
            hotspots.push(Hotspot::Misestimate { estimated, actual });
        }
    }
    if data.full_scan
        && data
            .examined_rows
            .is_none_or(|rows| rows >= LARGE_TABLE_ROWS)
    {
        hotspots.push(Hotspot::FullScan {
            rows: data.examined_rows,
        });
    }
    if let Some(reason) = &data.spill {
        hotspots.push(Hotspot::Spill {
            reason: reason.clone(),
        });
    }
    hotspots
}

/// Flag the top [`EXPENSIVE_TOP`] operators holding at least [`EXPENSIVE_SHARE`].
fn mark_expensive(nodes: &mut [AnalysedNode]) {
    let mut ranked: Vec<(usize, f64)> = nodes
        .iter()
        .filter(|node| node.share >= EXPENSIVE_SHARE)
        .map(|node| (node.id, node.share))
        .collect();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    for (id, share) in ranked.into_iter().take(EXPENSIVE_TOP) {
        if let Some(node) = nodes.get_mut(id) {
            node.hotspots.insert(0, Hotspot::Expensive { share });
        }
    }
}

#[cfg(test)]
#[path = "analysis_tests.rs"]
mod tests;
