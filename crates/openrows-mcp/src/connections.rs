// SPDX-License-Identifier: Apache-2.0
//! Enumerate only scoped connections whose current sharing gate permits disclosure.
use crate::{McpError, ToolContext, ToolInput, server::ToolServer};
use openrows_ai::sharing::{ContextBundle, GatedPayload, SchemaObject};
type Disclosure = (ToolContext, GatedPayload);
impl ToolServer {
    pub(crate) async fn connections(&self) -> Result<String, McpError> {
        let allowed = self.allowed_connections();
        let Some((context, first)) = allowed.first() else {
            return render(&[]);
        };
        // A single revision guard spans the complete multi-connection response.
        first
            .outbound(&context.provider, async {
                self.collect_connections(&allowed)
                    .await
                    .map_err(|_| openrows_ai::AiError::Disabled)
            })
            .await
            .map_err(|_| McpError::Sharing)
    }
    fn allowed_connections(&self) -> Vec<Disclosure> {
        self.grant
            .info
            .scope
            .connections
            .iter()
            .filter_map(|connection| {
                let input = ToolInput {
                    connection: Some(connection.clone()),
                    ..ToolInput::default()
                };
                let context = self
                    .backend
                    .context(&self.grant.info.scope.provider, &input)
                    .ok()?;
                let bundle = ContextBundle {
                    schema: vec![SchemaObject {
                        name: format!("connection.{connection}"),
                        ddl: String::new(),
                    }],
                    ..ContextBundle::default()
                };
                Some((context, self.gate(&input, bundle).ok()?))
            })
            .collect()
    }
    async fn collect_connections(&self, allowed: &[Disclosure]) -> Result<String, McpError> {
        let mut connections = Vec::new();
        for (context, gated) in allowed {
            let output = gated
                .outbound(&context.provider, async {
                    gated.record().await?;
                    Ok(gated.preview())
                })
                .await
                .map_err(|_| McpError::Sharing)?;
            let data = serde_json::from_str(&output.payload).map_err(|_| McpError::Invalid)?;
            connections.push(data);
        }
        self.live()?;
        render(&connections)
    }
}
fn render(connections: &[serde_json::Value]) -> Result<String, McpError> {
    let text =
        serde_json::to_string(&serde_json::json!({"untrustedData":{"connections":connections}}))
            .map_err(|_| McpError::Invalid)?;
    if text.len() > 16384 {
        return Err(McpError::Limit);
    }
    Ok(text)
}
