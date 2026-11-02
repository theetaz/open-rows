// SPDX-License-Identifier: Apache-2.0
//! Artifact tools return previews only; no backend save or execution method exists.
use crate::{McpError, ToolExecutor, ToolInput};
use openrows_ai::{
    agent::ToolReply,
    investigation::{ArtifactProposal, ArtifactReply, ProposedArtifact, Rejection},
    sharing::GatedPayload,
};
/// Assistant-only preview tools; MCP clients never see them.
pub(crate) const TOOLS: [&str; 3] = ["propose_board", "build_report", "record_health_finding"];
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
        let connection = input.connection.as_deref().ok_or(McpError::Invalid)?;
        let reply = match self.validate(tool, input, connection, context.dialect) {
            Ok(artifact) => ArtifactReply::Proposed(artifact),
            Err(reason) => ArtifactReply::Rejected {
                kind: kind(tool).into(),
                reason,
            },
        };
        admitted
            .guard(admitted.record())
            .await
            .map_err(|_| McpError::Sharing)?;
        Ok(ToolReply {
            context: admitted,
            proposal: None,
            failed: false,
            artifact: Some(reply),
        })
    }
    fn validate(
        &self,
        tool: &str,
        input: &ToolInput,
        connection: &str,
        dialect: openrows_sql::Dialect,
    ) -> Result<ProposedArtifact, Rejection> {
        let proposal: ArtifactProposal = serde_json::from_str(
            input
                .document
                .as_deref()
                .ok_or(Rejection::InvalidDocument)?,
        )
        .map_err(|_| Rejection::InvalidDocument)?;
        let matches = matches!(
            (tool, &proposal),
            ("propose_board", ArtifactProposal::Board { .. })
                | ("build_report", ArtifactProposal::Report { .. })
                | ("record_health_finding", ArtifactProposal::Finding { .. })
        );
        if !matches {
            return Err(Rejection::WrongKind);
        }
        self.ledger.propose(connection, dialect, proposal)
    }
}
fn kind(tool: &str) -> &'static str {
    match tool {
        "propose_board" => "board",
        "build_report" => "report",
        _ => "finding",
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
