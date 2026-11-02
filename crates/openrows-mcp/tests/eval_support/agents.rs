// SPDX-License-Identifier: Apache-2.0
//! The two agent-loop participants of the evaluation: the deterministic replay
//! provider and the `propose_sql`-only tool set.
use super::{Result, Task};
use openrows_ai::{
    AiControl, AiError, ProviderConfig, ProviderKind, RequestOptions, agent::*, sharing::*,
};
use openrows_net::BoxFuture;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

/// The evaluation provider's identity in the sharing allowlist.
pub fn provider_config(kind: ProviderKind, base_url: &str, keyed: bool) -> ProviderConfig {
    ProviderConfig {
        id: "eval".into(),
        name: "Evaluation".into(),
        kind,
        base_url: base_url.into(),
        allow_insecure_remote: false,
        secret_ref: if keyed {
            "ai/eval/api-key".into()
        } else {
            String::new()
        },
    }
}

/// Only `propose_sql`; a proposal is captured, never executed by the loop.
pub(super) struct ProposeTools {
    store: SharingStore,
    control: AiControl,
    config: ProviderConfig,
    dialect: openrows_sql::Dialect,
}
impl ProposeTools {
    pub(super) fn new(dialect: openrows_sql::Dialect) -> Result<Self> {
        let control = AiControl::default();
        control.set_enabled(true);
        let store = SharingStore::memory(1000)?;
        store.save(
            "eval",
            SharingPolicy {
                level: SharingLevel::SchemaOnly,
                providers: vec!["eval".into()],
                ..SharingPolicy::default()
            },
            false,
        )?;
        Ok(Self {
            store,
            control,
            config: provider_config(ProviderKind::OpenAi, "https://api.openai.com/v1/", true),
            dialect,
        })
    }
    pub(super) fn gate(&self, bundle: ContextBundle) -> Result<GatedPayload> {
        Ok(self.store.gate(GateRequest {
            connection: "eval",
            production: false,
            provider: &self.config,
            control: &self.control,
            dialect: self.dialect,
            bundle,
        })?)
    }
}
impl AgentTools for ProposeTools {
    fn definitions(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "propose_sql".into(),
            description: "Propose one read-only SQL query that answers the question. It is shown to the user, never run.".into(),
            schema: serde_json::json!({"type":"object","properties":{"sql":{"type":"string"}},"required":["sql"]}),
        }]
    }
    fn call<'a>(
        &'a self,
        call: &'a ToolCall,
    ) -> BoxFuture<'a, std::result::Result<ToolReply, AgentError>> {
        Box::pin(async move {
            let sql = call.arguments["sql"]
                .as_str()
                .ok_or(AgentError::Malformed)?;
            let context = self
                .gate(ContextBundle::default())
                .map_err(|_| AgentError::Refused)?;
            context.record().await?;
            Ok(ToolReply {
                failed: false,
                artifact: None,
                context,
                proposal: Some(sql.into()),
            })
        })
    }
}

/// Deterministic provider: replays the recorded proposal as one `propose_sql` call.
pub struct Replay {
    sql: String,
    proposed: AtomicBool,
}
impl Replay {
    pub fn new(task: &Task, schema: &str) -> Self {
        Self {
            sql: task.recorded.replace("{schema}", schema),
            proposed: AtomicBool::new(false),
        }
    }
}
impl AgentProvider for Replay {
    fn tool_calling(&self) -> bool {
        true
    }
    fn turn<'a>(
        &'a self,
        _: TurnRequest<'a>,
        _: mpsc::Sender<String>,
        _: &'a RequestOptions,
    ) -> BoxFuture<'a, std::result::Result<Turn, AiError>> {
        let first = !self.proposed.swap(true, Ordering::SeqCst);
        let calls = if first {
            vec![ToolCall {
                id: "replay".into(),
                name: "propose_sql".into(),
                arguments: serde_json::json!({ "sql": self.sql }),
            }]
        } else {
            vec![]
        };
        Box::pin(async move {
            Ok(Turn {
                calls,
                ..Turn::default()
            })
        })
    }
}
