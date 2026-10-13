// SPDX-License-Identifier: Apache-2.0
//! The shared executor refuses telemetry before collection and validates previews.
mod support;
use openrows_ai::{
    AiControl,
    agent::ToolCall,
    investigation::{self, Ledger},
    sharing::*,
};
use openrows_driver_api::BoxFuture;
use openrows_mcp::{McpError, ToolBackend, ToolContext, ToolExecutor, ToolInput};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use support::TestResult;
struct Backend {
    reads: AtomicU32,
    proposals: AtomicU32,
}
impl ToolBackend for Backend {
    fn context(&self, _: &str, _: &ToolInput) -> Result<ToolContext, McpError> {
        Ok(ToolContext {
            production: false,
            dialect: openrows_sql::Dialect::Postgres,
            provider: openrows_ai::ProviderConfig {
                id: "fixture".into(),
                name: "Fixture".into(),
                kind: openrows_ai::ProviderKind::Ollama,
                base_url: "http://127.0.0.1:1/".into(),
                allow_insecure_remote: false,
                secret_ref: String::new(),
            },
        })
    }
    fn read<'a>(
        &'a self,
        _: &'a str,
        _: &'a ToolInput,
    ) -> BoxFuture<'a, Result<ContextBundle, McpError>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Ok(ContextBundle {
                evidence: vec![investigation::sample(
                    "fixture:generation",
                    "sessions:active",
                    1000.0,
                    Some(4.0),
                )],
                ..ContextBundle::default()
            })
        })
    }
    fn propose(&self, _: &ToolInput) -> Result<(), McpError> {
        self.proposals.fetch_add(1, Ordering::SeqCst);
        Err(McpError::Unsafe)
    }
}
fn executor() -> TestResult<(ToolExecutor, Arc<Backend>)> {
    let backend = Arc::new(Backend {
        reads: AtomicU32::new(0),
        proposals: AtomicU32::new(0),
    });
    let control = AiControl::default();
    control.set_enabled(true);
    Ok((
        ToolExecutor {
            backend: backend.clone(),
            sharing: SharingStore::memory(10)?,
            control,
            ledger: Ledger::default(),
            provider: "fixture".into(),
            connections: vec!["connection".into()],
            database: Some("db".into()),
            external: false,
        },
        backend,
    ))
}
fn call(name: &str) -> ToolCall {
    ToolCall {
        id: "fixture".into(),
        name: name.into(),
        arguments: json!({"connection":"connection","database":"db"}),
    }
}
#[tokio::test]
async fn policy_refusals_precede_any_native_collection_for_every_monitor_tool() -> TestResult {
    let (executor, backend) = executor()?;
    for name in [
        "read_health_snapshot",
        "read_metric_history",
        "read_query_statistics",
        "read_lock_graph",
        "read_monitor_plans",
    ] {
        for level in [
            SharingLevel::None,
            SharingLevel::SchemaOnly,
            SharingLevel::SchemaAndStats,
        ] {
            executor.sharing.save(
                "connection",
                SharingPolicy {
                    level,
                    providers: vec!["fixture".into()],
                    telemetry: false,
                    ..SharingPolicy::default()
                },
                false,
            )?;
            assert!(executor.execute(&call(name)).await.is_err());
        }
    }
    assert_eq!(backend.reads.load(Ordering::SeqCst), 0);
    Ok(())
}
#[tokio::test]
async fn authorised_evidence_can_preview_but_cannot_save_or_execute_artifacts() -> TestResult {
    let (executor, backend) = executor()?;
    executor.sharing.save(
        "connection",
        SharingPolicy {
            level: SharingLevel::SchemaAndStats,
            providers: vec!["fixture".into()],
            telemetry: true,
            ..SharingPolicy::default()
        },
        false,
    )?;
    let reply = executor.execute(&call("read_health_snapshot")).await?;
    let input: ContextBundle = serde_json::from_str(&reply.context.preview().payload)?;
    let evidence = &input.evidence[0].id;
    let document = json!({"kind":"report","title":"Health","evidence":[evidence],"findings":[{"interpretation":"Activity increased","uncertainty":"Only one observation","evidence":[evidence]}],"chart":true});
    let mut proposal = call("build_report");
    proposal.arguments["document"] = json!(document.to_string());
    let artifact = executor
        .execute(&proposal)
        .await?
        .artifact
        .ok_or("artifact missing")?;
    let report: serde_json::Value = serde_json::from_str(artifact.document())?;
    assert_eq!(report["schema"], "openrows.monitor-report");
    assert_eq!(report["sections"][0]["rows"][0][4], 4.0);
    assert_eq!(backend.reads.load(Ordering::SeqCst), 1);
    proposal.arguments["document"]=json!(json!({"kind":"report","title":"Fake","evidence":["f".repeat(64)],"findings":[],"chart":false}).to_string());
    assert!(executor.execute(&proposal).await.is_err());
    assert_eq!(backend.proposals.load(Ordering::SeqCst), 0);
    Ok(())
}
#[tokio::test]
async fn unsafe_board_sql_and_external_artifact_calls_are_refused() -> TestResult {
    let (mut executor, backend) = executor()?;
    executor.sharing.save(
        "connection",
        SharingPolicy {
            level: SharingLevel::SchemaAndStats,
            providers: vec!["fixture".into()],
            telemetry: true,
            ..SharingPolicy::default()
        },
        false,
    )?;
    executor.execute(&call("read_health_snapshot")).await?;
    let id = executor.ledger.ids("connection")[0].clone();
    let mut board = json!({"version":1,"name":"Proposal","params":[],"refreshSecs":30,"widgets":[{"id":"chart","title":"Chart","kind":"table","sql":"DELETE FROM public.items","chart":null,"limits":{"maxRows":20,"timeoutMs":1000,"maxBytes":1024}}]});
    let mut proposal = call("propose_board");
    proposal.arguments["document"] =
        json!(json!({"kind":"board","board":board,"evidence":[id]}).to_string());
    assert!(executor.execute(&proposal).await.is_err());
    board["widgets"][0]["sql"] = json!("SELECT id FROM public.items");
    proposal.arguments["document"] =
        json!(json!({"kind":"board","board":board,"evidence":[id]}).to_string());
    let preview = executor
        .execute(&proposal)
        .await?
        .artifact
        .ok_or("missing board")?;
    assert_eq!(serde_json::to_value(&preview)?["evidence"], json!([id]));
    executor.external = true;
    assert!(executor.execute(&proposal).await.is_err());
    assert_eq!(backend.proposals.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn revocation_interrupts_pending_monitor_reads_without_recording_undisclosed_evidence()
-> TestResult {
    let backend = Arc::new(support::Backend::default());
    backend.wait.store(true, Ordering::SeqCst);
    let control = AiControl::default();
    control.set_enabled(true);
    let sharing = SharingStore::memory(10)?;
    sharing.save(
        "connection",
        SharingPolicy {
            level: SharingLevel::SchemaAndStats,
            providers: vec!["fixture".into()],
            telemetry: true,
            ..SharingPolicy::default()
        },
        false,
    )?;
    let executor = ToolExecutor {
        backend: backend.clone(),
        sharing: sharing.clone(),
        control,
        ledger: Ledger::default(),
        provider: "fixture".into(),
        connections: vec!["connection".into()],
        database: Some("db".into()),
        external: false,
    };
    let runner = executor.clone();
    let work = tokio::spawn(async move { runner.execute(&call("read_lock_graph")).await });
    backend.entered.notified().await;
    sharing.save("connection", SharingPolicy::default(), false)?;
    assert!(work.await?.is_err());
    assert!(executor.ledger.ids("connection").is_empty());
    assert!(sharing.audit()?.is_empty());
    Ok(())
}
