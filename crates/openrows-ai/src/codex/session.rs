// SPDX-License-Identifier: Apache-2.0
//! One ephemeral app-server thread per conversation turn: dynamic application
//! tools, streamed agent text and a fixed refusal for every approval request.
use super::{
    process::Launch,
    rpc::{Inbound, Rpc},
};
use crate::{
    AiError,
    agent::{Message, ToolCall, TurnRequest, calls::INSTRUCTIONS},
};
use serde_json::{Value, json};
use tokio::sync::mpsc;

const REFUSAL: &str = "OpenRows does not allow commands, file changes or extra permissions.";

/// Where a turn stopped.
pub(crate) enum Step {
    /// The CLI is waiting for these application tool results.
    Calls(Vec<ToolCall>),
    /// The turn completed.
    Finished,
}
pub(crate) struct Session {
    rpc: Rpc,
    thread: String,
    turn: String,
    pending: Vec<(String, Value)>,
}
impl Session {
    /// Start an ephemeral, read-only thread whose only extra tools are the
    /// application's, then send the gated context as the turn's input.
    pub(crate) async fn start(launch: &Launch, request: &TurnRequest<'_>) -> Result<Self, AiError> {
        let mut rpc = Rpc::connect(launch, true).await?;
        let work = launch.work();
        let work = work.to_str().ok_or(AiError::InvalidConfig)?;
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|tool| json!({"type": "function", "name": tool.name, "description": tool.description, "inputSchema": tool.schema}))
            .collect();
        let thread = rpc
            .request(
                "thread/start",
                json!({
                    "model": request.model, "cwd": work, "approvalPolicy": "on-request",
                    "sandbox": "read-only", "ephemeral": true,
                    "developerInstructions": INSTRUCTIONS, "dynamicTools": tools,
                }),
            )
            .await?;
        let thread = identifier(&thread["thread"]["id"])?;
        let turn = rpc
            .request(
                "turn/start",
                json!({
                    "threadId": thread, "model": request.model, "cwd": work,
                    "approvalPolicy": "on-request",
                    "sandboxPolicy": {"type": "readOnly", "networkAccess": false},
                    "input": [{"type": "text", "text": request.context.text(), "text_elements": []}],
                }),
            )
            .await?;
        let turn = identifier(&turn["turn"]["id"])?;
        Ok(Self {
            rpc,
            thread,
            turn,
            pending: Vec::new(),
        })
    }
    /// Answer the CLI's waiting tool calls with the loop's gated results.
    pub(crate) async fn reply(&mut self, messages: &[Message]) -> Result<(), AiError> {
        for (call, id) in std::mem::take(&mut self.pending) {
            let (data, failed) = messages
                .iter()
                .rev()
                .find_map(|message| match message {
                    Message::Tool {
                        id, data, failed, ..
                    } if *id == call => Some((data, *failed)),
                    _ => None,
                })
                .ok_or(AiError::InvalidResponse)?;
            let result =
                json!({"contentItems": [{"type": "inputText", "text": data}], "success": !failed});
            self.rpc.respond(id, result).await?;
        }
        Ok(())
    }
    /// Stream text until the CLI asks for an application tool or the turn ends.
    pub(crate) async fn drive(&mut self, sink: &mpsc::Sender<String>) -> Result<Step, AiError> {
        loop {
            let step = match self.rpc.next().await? {
                Inbound::Notification { method, params } => {
                    self.notification(&method, &params, sink).await?
                }
                Inbound::Request { id, method, params } if method == "item/tool/call" => {
                    let call = self.call(&params)?;
                    self.pending.push((call.id.clone(), id));
                    Some(Step::Calls(vec![call]))
                }
                Inbound::Request { id, method, .. } => {
                    refuse(&mut self.rpc, id, &method).await?;
                    None
                }
            };
            if let Some(step) = step {
                return Ok(step);
            }
        }
    }
    pub(crate) async fn close(self) {
        self.rpc.close().await;
    }
    fn current(&self, params: &Value) -> bool {
        params["threadId"] == self.thread.as_str()
            && (params["turnId"] == self.turn.as_str()
                || params["turn"]["id"] == self.turn.as_str())
    }
    async fn notification(
        &self,
        method: &str,
        params: &Value,
        sink: &mpsc::Sender<String>,
    ) -> Result<Option<Step>, AiError> {
        if !self.current(params) {
            return Ok(None);
        }
        match method {
            "item/agentMessage/delta" => {
                let delta = params["delta"].as_str().ok_or(AiError::InvalidResponse)?;
                if !delta.is_empty() {
                    sink.send(delta.to_owned())
                        .await
                        .map_err(|_| AiError::Cancelled)?;
                }
                Ok(None)
            }
            "turn/completed" => completed(&params["turn"]).map(Some),
            _ => Ok(None),
        }
    }
    fn call(&self, params: &Value) -> Result<ToolCall, AiError> {
        if params["threadId"] != self.thread.as_str()
            || params["turnId"] != self.turn.as_str()
            || !params["namespace"].is_null()
        {
            return Err(AiError::InvalidResponse);
        }
        let bounded = |value: &Value, limit: usize| {
            value
                .as_str()
                .filter(|text| !text.is_empty() && text.len() <= limit)
                .map(str::to_owned)
                .ok_or(AiError::InvalidResponse)
        };
        Ok(ToolCall {
            id: bounded(&params["callId"], 128)?,
            name: bounded(&params["tool"], 64)?,
            arguments: params["arguments"].clone(),
        })
    }
}
fn identifier(value: &Value) -> Result<String, AiError> {
    value
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .map(str::to_owned)
        .ok_or(AiError::InvalidResponse)
}
fn completed(turn: &Value) -> Result<Step, AiError> {
    match turn["status"].as_str() {
        Some("completed") => Ok(Step::Finished),
        Some("interrupted") => Err(AiError::Cancelled),
        Some("failed") => Err(failure(&turn["error"]["codexErrorInfo"])),
        _ => Err(AiError::InvalidResponse),
    }
}
/// Fixed categories for the CLI's typed turn errors; its messages are dropped.
fn failure(info: &Value) -> AiError {
    match info.as_str() {
        Some("unauthorized") => AiError::Auth,
        Some("usageLimitExceeded" | "sessionBudgetExceeded") => AiError::Quota,
        Some("rateLimitExceeded" | "serverOverloaded") => AiError::RateLimit { retry_after: None },
        Some("modelNotFound") => AiError::Unsupported,
        None if info.is_object() => AiError::Network,
        Some(_) | None => AiError::InvalidResponse,
    }
}
/// Decline every approval, elicitation and permission request; anything else
/// the bridge does not implement receives a JSON-RPC error.
pub(crate) async fn refuse(rpc: &mut Rpc, id: Value, method: &str) -> Result<(), AiError> {
    let result = match method {
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            json!({"decision": "decline"})
        }
        "execCommandApproval" | "applyPatchApproval" => {
            json!({"decision": {"denied": {"rejection": REFUSAL}}})
        }
        "mcpServer/elicitation/request" => {
            json!({"action": "decline", "content": null, "_meta": null})
        }
        "item/permissions/requestApproval" => json!({"permissions": {}, "scope": "turn"}),
        _ => return rpc.reject(id, REFUSAL).await,
    };
    rpc.respond(id, result).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn turn_outcomes_map_to_fixed_errors() {
        let turn = |status: &str, info: Value| json!({"status": status, "error": {"codexErrorInfo": info}});
        assert!(matches!(
            completed(&turn("completed", Value::Null)),
            Ok(Step::Finished)
        ));
        assert!(matches!(
            completed(&turn("interrupted", Value::Null)),
            Err(AiError::Cancelled)
        ));
        assert!(matches!(
            completed(&turn("failed", json!("unauthorized"))),
            Err(AiError::Auth)
        ));
        assert!(matches!(
            completed(&turn("failed", json!("usageLimitExceeded"))),
            Err(AiError::Quota)
        ));
        assert!(matches!(
            completed(&turn(
                "failed",
                json!({"httpConnectionFailed": {"httpStatusCode": null}})
            )),
            Err(AiError::Network)
        ));
        assert!(matches!(
            completed(&turn("inProgress", Value::Null)),
            Err(AiError::InvalidResponse)
        ));
    }
}
