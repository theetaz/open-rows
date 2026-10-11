// SPDX-License-Identifier: Apache-2.0
//! Bounded blocking graph with cycles, unknown owners and depth limits exposed.
use crate::{Column, MetricKind, Reading};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn enrich(reading: &mut Reading) {
    if reading
        .columns
        .get(5)
        .is_none_or(|column| column.name != "blockers")
    {
        return;
    }
    let mut edges = 0;
    let mut graph = BTreeMap::<String, Vec<String>>::new();
    for row in &mut reading.rows {
        let Some(pid) = row.first().cloned().flatten() else {
            continue;
        };
        let blockers = row.get(5).and_then(Option::as_deref).unwrap_or("");
        let ids: Vec<_> = blockers
            .trim_matches(['{', '}'])
            .split(',')
            .filter(|pid| !pid.is_empty())
            .take(100_usize.saturating_sub(edges))
            .map(str::to_owned)
            .collect();
        edges += ids.len();
        if edges >= 100 {
            reading.partial = true;
        }
        row[5] = Some(ids.join(","));
        graph.insert(pid, ids);
    }
    reading
        .columns
        .extend([column("blocking_chain"), column("chain_state")]);
    for row in &mut reading.rows {
        let pid = row.first().and_then(Option::as_deref).unwrap_or("");
        let (chain, state) = path(pid, &graph);
        row.extend([Some(chain), Some(state.into())]);
    }
}

fn path(pid: &str, graph: &BTreeMap<String, Vec<String>>) -> (String, &'static str) {
    let mut seen = BTreeSet::new();
    let mut pending = vec![(pid.to_owned(), 0)];
    let mut nodes = Vec::new();
    let mut state = "ok";
    while let Some((pid, depth)) = pending.pop() {
        if nodes.len() >= 100 {
            state = "edgeLimit";
            break;
        }
        if depth > 16 {
            state = "depthLimit";
            break;
        }
        if !seen.insert(pid.clone()) {
            state = "cycleOrRepeatedOwner";
            continue;
        }
        nodes.push(pid.clone());
        match graph.get(&pid) {
            Some(blockers) => pending.extend(
                blockers
                    .iter()
                    .take(100)
                    .map(|pid| (pid.clone(), depth + 1)),
            ),
            None => state = "unknownOwner",
        }
    }
    (nodes.join(" -> "), state)
}
fn column(name: &str) -> Column {
    Column {
        name: name.into(),
        unit: "pid".into(),
        kind: MetricKind::Text,
    }
}
