// SPDX-License-Identifier: Apache-2.0
//! User-triggered generation with a typed channel and the shared read-only tools.
mod context;
mod scheduled;
mod state;
use crate::{
    ai,
    connections::{IpcError, IpcErrorKind},
};
use openrows_ai::{
    AiError, RequestOptions,
    agent::{AgentError, AgentEvent, AgentLimits, AgentRequest, run_agent},
    sharing::{AssistantContext, ContextPreview, Plan},
};
pub(crate) use scheduled::*;
use serde::Deserialize;
use specta::Type;
pub(crate) use state::AssistantState;
use tauri::{AppHandle, Manager, ipc::Channel};

/// Selected tab context; native code resolves profile, permissions and provider secrets.
#[derive(Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AssistantRequest {
    pub(super) id: String,
    pub(super) provider: String,
    pub(super) model: String,
    pub(super) connection: String,
    pub(super) database: String,
    #[serde(default)]
    monitoring: bool,
    schema: String,
    question: AssistantContext,
    query: Option<String>,
    plan: Vec<Plan>,
}
impl AssistantRequest {
    fn validate(&self) -> Result<(), IpcError> {
        openrows_ai::validate_id(&self.id)?;
        openrows_ai::validate_id(&self.provider)?;
        if self.model.is_empty()
            || self.model.len() > 256
            || self.database.is_empty()
            || self.database.len() > 256
            || self.schema.is_empty()
            || self.schema.len() > 256
            || self.query.as_ref().is_some_and(|sql| sql.len() > 16384)
            || self.plan.len() > 200
        {
            return Err(AiError::InvalidConfig.into());
        }
        Ok(())
    }
}
impl From<AgentError> for IpcError {
    fn from(error: AgentError) -> Self {
        match error {
            AgentError::Provider(error) => error.into(),
            AgentError::Steps | AgentError::Tokens => {
                Self::new(IpcErrorKind::Timeout, error.to_string())
            }
            AgentError::Malformed => Self::new(IpcErrorKind::Protocol, error.to_string()),
            AgentError::Refused => Self::new(IpcErrorKind::Unsupported, error.to_string()),
        }
    }
}
/// Exact offline assistant preview from the same producer used for generation.
#[tauri::command]
#[specta::specta]
pub(crate) async fn assistant_preview(
    app: AppHandle,
    request: AssistantRequest,
) -> Result<ContextPreview, IpcError> {
    request.validate()?;
    let executor = context::executor(&app, &request)?;
    Ok(context::prepare(&executor, &request).await?.preview())
}
/// Explain a proposal only through the same protected plan tool; never ANALYZE.
#[tauri::command]
#[specta::specta]
pub(crate) async fn assistant_explain(
    app: AppHandle,
    request: AssistantRequest,
) -> Result<ContextPreview, IpcError> {
    request.validate()?;
    let executor = context::executor(&app, &request)?;
    let call = openrows_ai::agent::ToolCall {
        id: "explain".into(),
        name: "explain_query".into(),
        arguments: serde_json::json!({"connection":request.connection,"database":request.database,"sql":request.query.ok_or(AiError::InvalidConfig)?}),
    };
    Ok(openrows_ai::agent::AgentTools::call(&executor, &call)
        .await?
        .context
        .preview())
}
/// Cancel only this tab's current turn; cancellation remains available while locked.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
#[tauri::command]
#[specta::specta]
pub(crate) fn assistant_cancel(app: AppHandle, id: String) -> Result<(), IpcError> {
    openrows_ai::validate_id(&id)?;
    app.state::<AssistantState>().cancel(Some(&id));
    Ok(())
}
/// Stream one bounded investigation; SQL events have no execution path.
#[tauri::command]
#[specta::specta]
pub(crate) async fn assistant_run(
    app: AppHandle,
    request: AssistantRequest,
    on_event: Channel<AgentEvent>,
) -> Result<(), IpcError> {
    request.validate()?;
    let state = app.state::<AssistantState>();
    let (_run, cancel) = state.start(&request.id)?;
    let executor = context::executor(&app, &request)?;
    let options = RequestOptions {
        cancel,
        timeout: AgentLimits::default().timeout,
    };
    let work = Box::pin(async {
        let context = context::prepare(&executor, &request).await?;
        let provider = ai::generation_provider(&app, &request.provider, &request.model).await?;
        let input = AgentRequest {
            model: request.model.clone(),
            context,
            limits: AgentLimits::default(),
            options: options.clone(),
        };
        forward(&provider, &executor, &input, on_event).await
    });
    // Covers keychain/schema preparation as well as the agent loop.
    executor
        .control
        .run(&options, async { Ok(work.await) })
        .await??;
    Ok(())
}
async fn forward(
    provider: &dyn openrows_ai::agent::AgentProvider,
    tools: &dyn openrows_ai::agent::AgentTools,
    input: &AgentRequest,
    channel: Channel<AgentEvent>,
) -> Result<(), IpcError> {
    let (send, mut receive) = tokio::sync::mpsc::channel(16);
    let work = run_agent(provider, tools, input, send);
    tokio::pin!(work);
    loop {
        tokio::select! {
            biased;
            event=receive.recv()=>if let Some(event)=event {channel.send(event).map_err(|_|AiError::Cancelled)?;} else {break;},
            result=&mut work=>{result?;while let Ok(event)=receive.try_recv(){channel.send(event).map_err(|_|AiError::Cancelled)?;}return Ok(());}
        }
    }
    work.await?;
    Ok(())
}
