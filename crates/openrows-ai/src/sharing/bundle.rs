// SPDX-License-Identifier: Apache-2.0
//! Narrow context types exclude arbitrary plan metadata and raw telemetry.
use super::Category;
use serde::{Deserialize, Serialize};

/// Requested context before authorisation; never accepted by provider requests.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct ContextBundle {
    /// Explicit user questions and fixed error codes for assistant requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant: Option<AssistantContext>,
    /// Selected object definitions.
    pub schema: Vec<SchemaObject>,
    /// Statement to normalise, not a prompt or error message.
    pub query: Option<String>,
    /// Typed plan nodes without arbitrary vendor properties.
    pub plan: Vec<Plan>,
    /// Numeric query aggregates only, no users/addresses/parameters.
    pub stats: Vec<Aggregate>,
    /// Timestamped numeric monitoring evidence; no activity identities or raw text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<super::Evidence>,
    /// Explicit selected row samples.
    pub samples: Vec<Sample>,
}
impl ContextBundle {
    pub(super) fn validate_size(&self) -> Result<(), super::Refusal> {
        serde_json::to_writer(InputLimit(0), self).map_err(|_| super::Refusal::Invalid)
    }
    pub(super) fn categories(&self) -> Vec<Category> {
        [
            (self.assistant.is_some(), Category::Assistant),
            (!self.schema.is_empty(), Category::Schema),
            (self.query.is_some(), Category::Query),
            (!self.plan.is_empty(), Category::Plan),
            (!self.stats.is_empty(), Category::Stats),
            (!self.evidence.is_empty(), Category::Telemetry),
            (!self.samples.is_empty(), Category::Samples),
        ]
        .into_iter()
        .filter_map(|(present, category)| present.then_some(category))
        .collect()
    }
    pub(super) fn shrink(&mut self) -> Result<Option<usize>, super::Refusal> {
        if let Some(size) = pop_size(&mut self.samples)? {
            return Ok(Some(size));
        }
        if let Some(size) = pop_size(&mut self.evidence)? {
            return Ok(Some(size));
        }
        if let Some(size) = pop_size(&mut self.stats)? {
            return Ok(Some(size));
        }
        if let Some(size) = pop_size(&mut self.plan)? {
            return Ok(Some(size));
        }
        if let Some(size) = pop_size(&mut self.schema)? {
            return Ok(Some(size));
        }
        if let Some(query) = self.query.take() {
            return Ok(Some(
                serde_json::to_string(&query)
                    .map_err(|_| super::Refusal::Invalid)?
                    .len()
                    .saturating_sub(4),
            ));
        }
        Ok(None)
    }
}

/// User-authored questions, separate from untrusted database evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssistantContext {
    /// Bounded user question; previous user questions may supply follow-up context.
    pub question: String,
    /// Native-resolved tool scope; never a model-selected connection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<AssistantScope>,
    /// Previous user/assistant turns, bounded and re-redacted for every request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<AssistantHistory>,
    /// SQLSTATE/vendor code only; raw vendor messages never leave the app.
    pub error_code: Option<String>,
}

/// Native target identifiers are subject to the same sharing redactions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssistantScope {
    /// Saved connection identifier for tool arguments.
    pub connection: String,
    /// Selected database identifier for tool arguments.
    pub database: String,
    /// Native SQL dialect for proposals.
    pub dialect: String,
}
/// A prior displayed turn; it remains untrusted data rather than tool instructions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssistantHistory {
    /// Prior user question.
    pub question: String,
    /// Prior answer, with SQL literals and sensitive identifiers removed.
    pub answer: String,
}

/// Object identity plus DDL, whose literals and comments are removed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SchemaObject {
    /// Qualified object name.
    pub name: String,
    /// Definition only; no row estimates or statistics.
    pub ddl: String,
}

/// Whitelisted plan operators avoid retaining raw vendor output or literals.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PlanOperation {
    /// Scan.
    Scan,
    /// Join.
    Join,
    /// Sort.
    Sort,
    /// Aggregate.
    Aggregate,
    /// Other operator without vendor text.
    Other,
}

/// Flattened plan node; parent index preserves tree structure.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    /// Operator category.
    pub operation: PlanOperation,
    /// Parent node index, null at root.
    pub parent: Option<u32>,
    /// Qualified relation identifier.
    pub relation: String,
    /// Predicate SQL, normalised by the same lexer as query text.
    pub predicate: String,
    /// Numeric planner estimate.
    pub estimated_rows: u32,
}

/// Aggregates retain counts, never raw values or activity identities.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Aggregate {
    /// SQL normalised again at the outbound boundary.
    pub query: String,
    /// Executions.
    pub calls: u32,
    /// Total elapsed milliseconds.
    pub elapsed_ms: u32,
    /// Total processed rows.
    pub rows: u32,
}

/// A table's selected samples; values stay strings to avoid nested secret fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sample {
    /// Qualified table name.
    pub table: String,
    /// Column names in row order.
    pub columns: Vec<String>,
    /// Values in column order; null is SQL NULL.
    pub rows: Vec<Vec<Option<String>>>,
}

fn pop_size<T: Serialize>(items: &mut Vec<T>) -> Result<Option<usize>, super::Refusal> {
    items
        .pop()
        .map(|item| {
            let size = serde_json::to_vec(&item)
                .map_err(|_| super::Refusal::Invalid)?
                .len();
            Ok(size + usize::from(!items.is_empty()))
        })
        .transpose()
}

struct InputLimit(usize);
impl std::io::Write for InputLimit {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        if self.0 > 1024 * 1024 {
            return Err(std::io::Error::other("context input limit"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
