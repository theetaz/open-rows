// SPDX-License-Identifier: Apache-2.0
//! Resolve scope and collect bounded native schema through the shared MCP backend.
use super::AssistantRequest;
use crate::{ai::AiState, connections::IpcError};
use openrows_ai::sharing::{AssistantScope, ContextBundle, GatedPayload, SharingStore};
use openrows_mcp::{ToolExecutor, ToolInput};
use std::{sync::Arc, time::Duration};
use tauri::{AppHandle, Manager};
pub(super) fn executor(
    app: &AppHandle,
    request: &AssistantRequest,
) -> Result<ToolExecutor, IpcError> {
    app.state::<crate::security::Security>()
        .admit("assistant_run")?;
    let value =
        crate::settings::current(&app.state::<crate::settings::SettingsFile>()).unwrap_or_default();
    app.state::<AiState>().configure(&value);
    Ok(ToolExecutor {
        ledger: openrows_ai::investigation::Ledger::default(),
        backend: Arc::new(crate::mcp::backend::Backend(app.clone())),
        sharing: app.state::<SharingStore>().inner().clone(),
        control: app.state::<AiState>().control.clone(),
        provider: request.provider.clone(),
        connections: vec![request.connection.clone()],
        database: Some(request.database.clone()),
        external: false,
    })
}
pub(super) async fn prepare(
    executor: &ToolExecutor,
    request: &AssistantRequest,
) -> Result<GatedPayload, IpcError> {
    tokio::time::timeout(Duration::from_secs(5), collect(executor, request))
        .await
        .map_err(|_| openrows_ai::AiError::Timeout)?
}
pub(super) async fn collect(
    executor: &ToolExecutor,
    request: &AssistantRequest,
) -> Result<GatedPayload, IpcError> {
    let mut bundle = ContextBundle {
        assistant: Some(request.question.clone()),
        query: request.query.clone(),
        plan: request.plan.clone(),
        ..ContextBundle::default()
    };
    let input = ToolInput {
        connection: Some(request.connection.clone()),
        database: Some(request.database.clone()),
        schema: Some(request.schema.clone()),
        ..ToolInput::default()
    };
    let native = executor.backend.context(&request.provider, &input)?;
    if let Some(assistant) = &mut bundle.assistant {
        assistant.scope = Some(AssistantScope {
            connection: request.connection.clone(),
            database: request.database.clone(),
            dialect: format!("{:?}", native.dialect).to_ascii_lowercase(),
        });
    }
    let admitted = executor.gate(&input, bundle.clone())?;
    if request.monitoring {
        let permission = ContextBundle {
            evidence: vec![openrows_ai::investigation::sample(
                "admission",
                "telemetry:permission",
                0.0,
                None,
            )],
            ..bundle.clone()
        };
        let admitted = executor.gate(&input, permission)?;
        bundle.evidence = read(executor, &admitted, "read_health_snapshot", &input)
            .await?
            .evidence;
        let payload = executor.gate(&input, bundle)?;
        executor.ledger.record(&request.connection, &payload)?;
        return Ok(payload);
    }
    bundle.schema = read(executor, &admitted, "list_objects", &input)
        .await?
        .schema;
    definitions(executor, &admitted, &input, &mut bundle).await?;
    Ok(executor.gate(&input, bundle)?)
}
async fn definitions(
    executor: &ToolExecutor,
    admitted: &GatedPayload,
    input: &ToolInput,
    bundle: &mut ContextBundle,
) -> Result<(), IpcError> {
    // Include definitions for the first eight objects; both preview and send use
    // this exact producer, with all fields rechecked/redacted by the final gate.
    for object in bundle.schema.iter().take(8).cloned().collect::<Vec<_>>() {
        let Some((schema, name)) = object.name.split_once('.') else {
            continue;
        };
        let selected = ToolInput {
            schema: Some(schema.into()),
            object: Some(name.into()),
            ..input.clone()
        };
        let details = read(executor, admitted, "describe_object", &selected).await?;
        if let Some(definition) = details.schema.first()
            && let Some(target) = bundle
                .schema
                .iter_mut()
                .find(|item| item.name == definition.name)
        {
            target.ddl.clone_from(&definition.ddl);
        }
    }
    Ok(())
}
async fn read(
    executor: &ToolExecutor,
    admitted: &GatedPayload,
    tool: &str,
    input: &ToolInput,
) -> Result<ContextBundle, IpcError> {
    Ok(admitted
        .guard(async {
            Ok(
                tokio::time::timeout(Duration::from_secs(5), executor.backend.read(tool, input))
                    .await,
            )
        })
        .await?
        .map_err(|_| openrows_ai::AiError::Timeout)??)
}
