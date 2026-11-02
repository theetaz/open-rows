// SPDX-License-Identifier: Apache-2.0
//! Default-deny database context, bounded redaction and metadata-only audit.
mod bundle;
mod evidence;
mod investigation_audit;
pub use evidence::{Evidence, EvidenceState};
pub use investigation_audit::InvestigationAudit;
mod gate;
mod redact;
mod store;
mod types;

pub use bundle::{
    Aggregate, AssistantContext, AssistantHistory, AssistantScope, ContextBundle, Plan,
    PlanOperation, Sample, SchemaObject,
};
pub use gate::{GateRequest, GatedPayload};
pub use store::SharingStore;
pub use types::{
    AuditEntry, Category, ContextPreview, Limits, RedactionRule, Refusal, SharingLevel,
    SharingPolicy,
};
