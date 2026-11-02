// SPDX-License-Identifier: Apache-2.0
//! Fixed assistant events and a narrow provider/tool boundary.
use crate::{AiError, RequestOptions, sharing::GatedPayload};
use openrows_net::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

/// Static tool description; database data cannot register tools.
#[derive(Clone, Debug, Serialize)]
pub struct ToolDefinition {
    /// Fixed application tool name.
    pub name: String,
    /// Fixed application description.
    pub description: String,
    /// Application-owned input schema.
    pub schema: Value,
}
/// One provider-requested tool invocation, validated before execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    /// Provider's opaque correlation ID.
    pub id: String,
    /// Application tool name.
    pub name: String,
    /// Complete JSON arguments; never execute partial streaming JSON.
    pub arguments: Value,
}
/// Bounded provider-owned continuation data; it cannot register tools or set policy.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Continuation {
    pub(crate) responses: Vec<Value>,
    pub(crate) gemini: Vec<Value>,
    pub(crate) chat: Vec<Value>,
    pub(crate) chat_calls: std::collections::BTreeMap<String, Value>,
}
/// Only the loop constructs conversation messages; tool results require gated payloads.
#[derive(Clone, Debug)]
pub enum Message {
    /// Model text and explicit protocol tool calls.
    Assistant {
        /// Model text, kept separate from tool results.
        text: String,
        /// Completed protocol calls.
        calls: Vec<ToolCall>,
        /// Opaque protocol signatures/reasoning, retained only within this request.
        continuation: Continuation,
    },
    /// Gate-filtered tool data, carried in the protocol's result role.
    Tool {
        /// Protocol correlation ID.
        id: String,
        /// Fixed tool name.
        name: String,
        /// Gate-filtered result JSON or fixed refusal.
        data: String,
        /// Whether policy refused the tool.
        failed: bool,
    },
}
/// Borrowed inputs for one bounded provider round.
pub struct TurnRequest<'a> {
    /// User-selected model.
    pub(crate) model: &'a str,
    /// Mandatory current policy guard and exact initial context.
    pub(crate) context: &'a GatedPayload,
    /// Loop-owned previous model/tool messages.
    pub(crate) messages: &'a [Message],
    /// Static tools, empty for a single-shot fallback.
    pub(crate) tools: &'a [ToolDefinition],
    /// Remaining conservative output budget.
    pub(crate) max_output: u32,
}
impl TurnRequest<'_> {
    /// Loop-owned conversation for scripted providers and bridge adapters.
    #[must_use]
    pub fn messages(&self) -> &[Message] {
        self.messages
    }
    /// Exact gated context for provider adapters.
    #[must_use]
    pub fn preview(&self) -> crate::sharing::ContextPreview {
        self.context.preview()
    }
}
/// Tool calls arrive only as a completed provider turn, never parsed from text.
#[derive(Clone, Debug, Default)]
pub struct Turn {
    /// Explicit protocol calls.
    pub calls: Vec<ToolCall>,
    /// Bounded native continuation metadata for the same provider/model.
    pub continuation: Continuation,
}
/// Provider-independent streaming and tool calling.
pub trait AgentProvider: Send + Sync {
    /// Whether the configured model explicitly supports tools; unknown falls back.
    fn tool_calling(&self) -> bool;
    /// Stream text deltas and return complete tool calls.
    fn turn<'a>(
        &'a self,
        request: TurnRequest<'a>,
        sink: mpsc::Sender<String>,
        options: &'a RequestOptions,
    ) -> BoxFuture<'a, Result<Turn, AiError>>;
}
/// Successful tool data can only come from the shared policy gate.
pub struct ToolReply {
    /// Exact redacted tool result.
    pub context: GatedPayload,
    /// Protocol tool failure with a policy-filtered result.
    pub failed: bool,
    /// SQL proposal for local user review; no execution action exists.
    pub proposal: Option<String>,
}
/// Shared read/propose executor; there is no write capability.
pub trait AgentTools: Send + Sync {
    /// Application-owned tool catalogue.
    fn definitions(&self) -> Vec<ToolDefinition>;
    /// Execute one validated, authorised read or create a review proposal.
    fn call<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<ToolReply, AgentError>>;
}
/// Fixed failures; raw database/provider errors never reach the trace.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AgentError {
    /// Transport's existing typed failure.
    #[error(transparent)]
    Provider(#[from] AiError),
    /// Total provider/tool step budget reached.
    #[error("Assistant step limit reached")]
    Steps,
    /// Conservative byte/token budget reached.
    #[error("Assistant token limit reached")]
    Tokens,
    /// Invalid call or unsupported tool.
    #[error("Assistant returned a malformed tool call")]
    Malformed,
    /// Read, policy or safety refusal.
    #[error("Tool refused by sharing or execution policy")]
    Refused,
}
/// A tool trace contains safe summaries and policy-filtered external evidence.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Trace {
    /// Sequential step number.
    pub step: u32,
    /// Fixed tool name.
    pub tool: String,
    /// Argument field names and byte lengths, without values or SQL literals.
    pub arguments: String,
    /// Exact gated result size.
    pub bytes: u32,
    /// Gate redaction/truncation disclosure.
    pub redacted: bool,
    /// Elapsed milliseconds.
    pub duration_ms: u32,
    /// Redacted external tool evidence only; never raw protocol data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Running/succeeded/refused/cancelled.
    pub status: String,
}
/// Ordered, bounded UI events; plain text is never interpreted as HTML.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    /// Streaming model text.
    Text {
        /// Incremental plain text.
        delta: String,
    },
    /// Visible call start/completion.
    Trace {
        /// Safe tool metadata.
        trace: Trace,
    },
    /// User-review-only SQL.
    Proposal {
        /// Literal SQL for user review.
        sql: String,
    },
    /// Native first-use prompt for an enabled user-server tool.
    Confirmation {
        /// Ephemeral pending consent ID.
        id: String,
        /// User-configured display name.
        server: String,
        /// Discovered enabled tool.
        tool: String,
        /// Exact policy-filtered arguments, never credentials.
        arguments: String,
    },
    /// Successful completion and conservative usage.
    Done {
        /// Conservative total token upper bound.
        tokens: u32,
    },
}
