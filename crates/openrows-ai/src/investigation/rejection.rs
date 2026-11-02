// SPDX-License-Identifier: Apache-2.0
//! Typed proposal refusals; fixed text, never model or database content.
use super::ProposedArtifact;
use serde::Serialize;

/// Why an assistant board, report or finding proposal was not shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Rejection {
    /// JSON does not match the board, report or finding schema.
    #[error("the proposal does not match the board, report or finding schema")]
    InvalidDocument,
    /// The document's kind differs from the tool that was called.
    #[error("the proposal kind does not match the tool that was called")]
    WrongKind,
    /// No citation, or more than 100.
    #[error("the proposal must cite between 1 and 100 evidence ids")]
    CitationCount,
    /// A citation was never delivered in this investigation.
    #[error("the proposal cites evidence that was not delivered in this investigation")]
    UnknownEvidence,
    /// A report finding cites evidence missing from the report's own list.
    #[error("a finding cites evidence that the report does not include")]
    UncitedFinding,
    /// A board widget's SQL is not a validated single read.
    #[error("a board widget's SQL is not a validated read-only query")]
    UnsafeSql,
    /// Empty, oversized or NUL-containing title or interpretation.
    #[error("a title, interpretation or uncertainty is empty or longer than 2,048 bytes")]
    InvalidText,
    /// Document, finding or chart limits exceeded.
    #[error("the proposal exceeds the document, finding or chart limits")]
    TooLarge,
}
/// Result of an assistant-only artifact tool: a preview or a typed refusal.
#[derive(Clone, Debug)]
pub enum ArtifactReply {
    /// Validated preview; only an explicit local user action can save it.
    Proposed(ProposedArtifact),
    /// Refused proposal; the user and the model see the typed reason.
    Rejected {
        /// board, report or finding.
        kind: String,
        /// Fixed refusal.
        reason: Rejection,
    },
}
