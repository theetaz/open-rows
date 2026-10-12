// SPDX-License-Identifier: Apache-2.0
//! Artifact tools return previews only; no backend save or execution method exists.
use crate::{McpError, ToolExecutor, ToolInput};
use openrows_ai::{agent::ToolReply, investigation::ArtifactProposal, sharing::GatedPayload};
impl ToolExecutor {
    pub(crate) async fn artifact(
        &self,
        tool: &str,
        input: &ToolInput,
        admitted: GatedPayload,
    ) -> Result<ToolReply, McpError> {
        if self.external {
            return Err(McpError::Invalid);
        }
        let context = self.backend.context(&self.provider, input)?;
        let proposal: ArtifactProposal =
            serde_json::from_str(input.document.as_deref().ok_or(McpError::Invalid)?)
                .map_err(|_| McpError::Invalid)?;
        let matches = matches!(
            (tool, &proposal),
            ("propose_board", ArtifactProposal::Board { .. })
                | ("build_report", ArtifactProposal::Report { .. })
                | ("record_health_finding", ArtifactProposal::Finding { .. })
        );
        if !matches {
            return Err(McpError::Invalid);
        }
        let artifact = self
            .ledger
            .propose(
                input.connection.as_deref().ok_or(McpError::Invalid)?,
                context.dialect,
                proposal,
            )
            .map_err(|_| McpError::Unsafe)?;
        admitted
            .guard(admitted.record())
            .await
            .map_err(|_| McpError::Sharing)?;
        Ok(ToolReply {
            context: admitted,
            proposal: None,
            artifact: Some(artifact),
        })
    }
}

/// Cite the existing protected plan result without retaining raw SQL in the evidence.
pub(crate) fn plan_evidence(
    input: &ToolInput,
    bundle: &mut openrows_ai::sharing::ContextBundle,
) -> Result<(), McpError> {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| McpError::Invalid)?
        .as_secs_f64()
        * 1000.0;
    let binding = format!(
        "{}:{}:{}",
        input.connection.as_deref().ok_or(McpError::Invalid)?,
        input.database.as_deref().ok_or(McpError::Invalid)?,
        input.sql.as_deref().ok_or(McpError::Invalid)?
    );
    bundle.evidence = bundle
        .plan
        .iter()
        .enumerate()
        .map(|(index, node)| {
            openrows_ai::investigation::sample(
                &format!("{binding}:{index}"),
                "plan:estimated_rows",
                time,
                Some(f64::from(node.estimated_rows)),
            )
        })
        .collect();
    Ok(())
}
