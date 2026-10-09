// SPDX-License-Identifier: Apache-2.0
//! Phase one of a spill pass: which batches leave RAM.
use crate::set::ResultSet;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use std::collections::HashMap;

/// Batches of one result chosen to spill, with their slot indices.
pub(crate) struct Victim {
    pub(crate) id: String,
    pub(crate) generation: u64,
    pub(crate) schema: SchemaRef,
    pub(crate) batches: Vec<(usize, RecordBatch)>,
}

/// Enough in-memory batches to free `excess` bytes: whole results in
/// least-recently-used order, each result's oldest batches first, so the
/// result being viewed or streamed is spilled last.
pub(crate) fn pick(results: &HashMap<String, ResultSet>, mut excess: usize) -> Vec<Victim> {
    let mut order: Vec<(&String, &ResultSet)> = results
        .iter()
        .filter(|(_, result)| result.bytes > 0)
        .collect();
    order.sort_by_key(|(_, result)| result.last_used());
    let mut victims = Vec::new();
    for (id, result) in order {
        if excess == 0 {
            break;
        }
        let mut batches = Vec::new();
        for (index, batch, bytes) in result.in_memory() {
            if excess == 0 {
                break;
            }
            excess = excess.saturating_sub(bytes);
            batches.push((index, batch.clone()));
        }
        victims.push(Victim {
            id: id.clone(),
            generation: result.generation,
            schema: result.schema.clone(),
            batches,
        });
    }
    victims
}
