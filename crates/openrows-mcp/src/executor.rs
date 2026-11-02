// SPDX-License-Identifier: Apache-2.0
//! One scoped implementation for MCP and the in-app assistant, including sharing.
use crate::{McpError, ToolBackend, ToolInput, tools};
use openrows_ai::{
    AiControl,
    agent::{AgentError, AgentTools, ToolCall, ToolDefinition, ToolReply},
    sharing::{
        ContextBundle, GateRequest, GatedPayload, Plan, PlanOperation, Sample, SchemaObject,
        SharingStore,
    },
};
use openrows_driver_api::BoxFuture;
use std::sync::Arc;

/// Native identity and scope, never supplied by model tool arguments.
#[derive(Clone)]
pub struct ToolExecutor {
    /// Existing app catalog/protected-read/proposal implementation.
    pub backend: Arc<dyn ToolBackend>,
    /// Device policy and mandatory audit.
    pub sharing: SharingStore,
    /// Live global permission.
    pub control: AiControl,
    /// Saved provider ID.
    pub provider: String,
    /// Explicit connection scope.
    pub connections: Vec<String>,
    /// Assistant's selected database; MCP may select among open databases.
    pub database: Option<String>,
    /// External MCP cannot attest local-only inference.
    pub external: bool,
}
impl ToolExecutor {
    /// Apply the same context gate as a tool response or offline preview.
    /// # Errors
    /// Refuses off/locked/closed/out-of-scope/disallowed context.
    pub fn gate(&self, input: &ToolInput, bundle: ContextBundle) -> Result<GatedPayload, McpError> {
        let id = input.connection.as_deref().ok_or(McpError::Invalid)?;
        if !self.connections.iter().any(|connection| connection == id)
            || self
                .database
                .as_ref()
                .is_some_and(|database| input.database.as_ref() != Some(database))
        {
            return Err(McpError::Auth);
        }
        let context = self.backend.context(&self.provider, input)?;
        if self.external
            && self
                .sharing
                .policy(id)
                .map_err(|_| McpError::Sharing)?
                .local_only
        {
            return Err(McpError::Sharing);
        }
        self.sharing
            .gate(GateRequest {
                connection: id,
                production: context.production,
                provider: &context.provider,
                control: &self.control,
                dialect: context.dialect,
                bundle,
            })
            .map_err(|_| McpError::Sharing)
    }
    /// Call a statically defined tool with identical validation and native safety checks.
    /// # Errors
    /// Typed refusal, cancellation, malformed input or bounded database failure.
    pub async fn execute(&self, call: &ToolCall) -> Result<ToolReply, McpError> {
        let arguments = call
            .arguments
            .as_object()
            .cloned()
            .ok_or(McpError::Invalid)?;
        let input = tools::input(&call.name, Some(arguments))?;
        if call.name == "list_connections" {
            return self.connections().await;
        }
        let admitted = self.gate(&input, minimum(&call.name))?;
        let context = self.backend.context(&self.provider, &input)?;
        if matches!(call.name.as_str(), "run_read_query" | "explain_query") {
            openrows_sql::validate_ai_read(
                input.sql.as_deref().ok_or(McpError::Invalid)?,
                context.dialect,
            )
            .map_err(|_| McpError::Unsafe)?;
        }
        let bundle = if call.name == "propose_sql" {
            ContextBundle {
                query: input.sql.clone(),
                ..ContextBundle::default()
            }
        } else {
            admitted
                .outbound(&context.provider, async {
                    Ok(self.backend.read(&call.name, &input).await)
                })
                .await
                .map_err(|_| McpError::Cancelled)??
        };
        let gated = self.gate(&input, bundle)?;
        gated
            .outbound(&context.provider, async { gated.record().await })
            .await
            .map_err(|_| McpError::Sharing)?;
        let proposal = if call.name == "propose_sql" {
            if self.external {
                self.backend.propose(&input)?;
                None
            } else {
                input.sql.clone()
            }
        } else {
            None
        };
        Ok(ToolReply {
            failed: false,
            context: gated,
            proposal,
        })
    }
    async fn connections(&self) -> Result<ToolReply, McpError> {
        let id = self.connections.first().ok_or(McpError::Auth)?;
        let input = ToolInput {
            connection: Some(id.clone()),
            database: self.database.clone(),
            ..ToolInput::default()
        };
        let context = self.gate(
            &input,
            ContextBundle {
                schema: vec![SchemaObject {
                    name: format!("connection.{id}"),
                    ddl: String::new(),
                }],
                ..ContextBundle::default()
            },
        )?;
        context
            .guard(context.record())
            .await
            .map_err(|_| McpError::Sharing)?;
        Ok(ToolReply {
            failed: false,
            context,
            proposal: None,
        })
    }
}
impl AgentTools for ToolExecutor {
    fn definitions(&self) -> Vec<ToolDefinition> {
        tools::definitions()
            .into_iter()
            .map(|tool| ToolDefinition {
                name: tool.name.to_string(),
                description: tool
                    .description
                    .map_or_else(String::new, |text| text.to_string()),
                schema: serde_json::Value::Object((*tool.input_schema).clone()),
            })
            .collect()
    }
    fn call<'a>(&'a self, call: &'a ToolCall) -> BoxFuture<'a, Result<ToolReply, AgentError>> {
        Box::pin(async move {
            tokio::time::timeout(std::time::Duration::from_secs(5), self.execute(call))
                .await
                .map_err(|_| AgentError::Provider(openrows_ai::AiError::Timeout))?
                .map_err(|error| match error {
                    McpError::Invalid => AgentError::Malformed,
                    McpError::Cancelled => AgentError::Provider(openrows_ai::AiError::Cancelled),
                    _ => AgentError::Refused,
                })
        })
    }
}
fn minimum(tool: &str) -> ContextBundle {
    let mut bundle = ContextBundle::default();
    if tool == "run_read_query" {
        bundle.samples.push(Sample {
            table: "result".into(),
            columns: vec!["value".into()],
            rows: vec![],
        });
    }
    if tool == "explain_query" {
        bundle.plan.push(Plan {
            operation: PlanOperation::Other,
            parent: None,
            relation: String::new(),
            predicate: String::new(),
            estimated_rows: 0,
        });
    }
    bundle
}
