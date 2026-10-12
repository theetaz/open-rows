// SPDX-License-Identifier: Apache-2.0
//! Only delivered evidence can back a finding or measured report value.
use crate::{
    agent::AgentError,
    sharing::{ContextBundle, Evidence, GatedPayload},
};
use openrows_monitor::boards::{Board, parse_board};
use openrows_sql::Dialect;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

/// A model interpretation, never a measured fact.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Finding {
    /// Plain text interpretation for user review.
    pub interpretation: String,
    /// Explicit uncertainty, not a promised improvement.
    pub uncertainty: String,
    /// Only evidence identifiers actually delivered in this investigation.
    pub evidence: Vec<String>,
}
/// Application-owned proposal schema; measured values cannot be supplied by a model.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ArtifactProposal {
    /// Read-only widgets using the shared board model.
    Board {
        /// Shared strict board document.
        board: Board,
        /// Delivered evidence references.
        evidence: Vec<String>,
    },
    /// A shared report model built from recorded evidence, not fabricated points.
    Report {
        /// Plain display title.
        title: String,
        /// Interpreted findings with uncertainty.
        findings: Vec<Finding>,
        /// Measured samples selected for the report.
        evidence: Vec<String>,
        /// Include a validated scatter chart of actual samples.
        chart: bool,
    },
    /// Evidence-grounded plain text health finding.
    Finding {
        /// Grounded plain text interpretation.
        finding: Finding,
    },
}
/// Validated preview for the local UI; no save/run action is representable.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedArtifact {
    /// board, report or finding.
    kind: String,
    /// Shared versioned document JSON, never HTML/script.
    document: String,
    /// Validated citations kept beside portable documents that have no citation field.
    evidence: Vec<String>,
}
impl ProposedArtifact {
    /// The validated versioned preview; no public constructor or deserializer exists.
    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }
    pub(crate) fn into_parts(self) -> (String, String, Vec<String>) {
        (self.kind, self.document, self.evidence)
    }
}
/// Bounded request-local evidence, keyed by connection to prevent cross-scope citations.
#[derive(Clone, Debug, Default)]
pub struct Ledger(Arc<Mutex<BTreeMap<(String, String), Evidence>>>);
impl Ledger {
    /// Record the exact gate output after truncation/redaction, not the raw input.
    /// # Errors
    /// Refuses damaged gated JSON.
    pub fn record(&self, connection: &str, payload: &GatedPayload) -> Result<(), AgentError> {
        let bundle: ContextBundle =
            serde_json::from_str(&payload.preview().payload).map_err(|_| AgentError::Malformed)?;
        let mut items = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        for point in bundle.evidence {
            if items.len() >= 200 {
                break;
            }
            items.insert((connection.into(), point.id.clone()), point);
        }
        Ok(())
    }
    /// Exact retained opaque references, for scrubbed accounting only.
    #[must_use]
    pub fn ids(&self, connection: &str) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .filter(|(scope, _)| scope == connection)
            .map(|(_, id)| id.clone())
            .collect()
    }
    fn points(&self, connection: &str, ids: &[String]) -> Result<Vec<Evidence>, AgentError> {
        if ids.is_empty() || ids.len() > 100 {
            return Err(AgentError::Refused);
        }
        let items = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        ids.iter()
            .map(|id| {
                items
                    .get(&(connection.into(), id.clone()))
                    .cloned()
                    .ok_or(AgentError::Refused)
            })
            .collect()
    }
    /// Validate citations and read-only SQL before creating a preview.
    /// # Errors
    /// Refuses fabricated references, malformed documents and unsafe widget SQL.
    pub fn propose(
        &self,
        connection: &str,
        dialect: Dialect,
        proposal: ArtifactProposal,
    ) -> Result<ProposedArtifact, AgentError> {
        let (kind, document, evidence) = match proposal {
            ArtifactProposal::Board { board, evidence } => {
                self.points(connection, &evidence)?;
                let text = serde_json::to_string(&board).map_err(|_| AgentError::Malformed)?;
                parse_board(&text).map_err(|_| AgentError::Malformed)?;
                for widget in &board.widgets {
                    openrows_sql::validate_ai_read(&widget.sql, dialect)
                        .map_err(|_| AgentError::Refused)?;
                }
                ("board", text, evidence)
            }
            ArtifactProposal::Finding { finding } => {
                self.finding(connection, &finding)?;
                (
                    "finding",
                    serde_json::to_string(&finding).map_err(|_| AgentError::Malformed)?,
                    finding.evidence,
                )
            }
            ArtifactProposal::Report {
                title,
                findings,
                evidence,
                chart,
            } => {
                let points = self.points(connection, &evidence)?;
                text(&title)?;
                if findings.len() > 16 {
                    return Err(AgentError::Malformed);
                }
                for finding in &findings {
                    self.finding(connection, finding)?;
                    if !finding.evidence.iter().all(|id| evidence.contains(id)) {
                        return Err(AgentError::Refused);
                    }
                }
                (
                    "report",
                    report(&title, &findings, &points, (chart, generated_ms()?))?.to_string(),
                    evidence,
                )
            }
        };
        if document.len() > 65536 {
            return Err(AgentError::Malformed);
        }
        Ok(ProposedArtifact {
            kind: kind.into(),
            document,
            evidence,
        })
    }
    fn finding(&self, connection: &str, finding: &Finding) -> Result<(), AgentError> {
        text(&finding.interpretation)?;
        text(&finding.uncertainty)?;
        self.points(connection, &finding.evidence).map(drop)
    }
}
fn text(value: &str) -> Result<(), AgentError> {
    if value.trim().is_empty() || value.len() > 2048 || value.contains('\0') {
        return Err(AgentError::Malformed);
    }
    Ok(())
}
fn report(
    title: &str,
    findings: &[Finding],
    points: &[Evidence],
    (chart, generated): (bool, f64),
) -> Result<serde_json::Value, AgentError> {
    let from = points.iter().map(|p| p.collected_ms).min_by(f64::total_cmp);
    let to = points.iter().map(|p| p.collected_ms).max_by(f64::total_cmp);
    let evidence = json!({"source":"Authorised AI investigation", "fromMs":from, "toMs":to, "scope":"Selected connection", "gaps":["Only supplied bounded observations are included; redaction or truncation may omit data. Model interpretations require review; sample states retain gaps and visibility."]});
    let mut sections = vec![
        json!({"id":"evidence", "title":"Measured evidence", "kind":"table", "evidence":evidence, "columns":[{"name":"id","unit":""},{"name":"metric","unit":""},{"name":"collected","unit":"ms"},{"name":"scope","unit":""},{"name":"value","unit":"source unit"},{"name":"state","unit":""}], "rows":points.iter().map(|p|json!([p.id,p.metric,p.collected_ms,p.scope,p.value,serde_json::to_value(p.state).unwrap_or_default()])).collect::<Vec<_>>()}),
    ];
    for (index, finding) in findings.iter().enumerate() {
        sections.push(json!({"id":format!("finding-{index}"), "title":"Model interpretation (unverified)", "kind":"summary", "evidence":evidence, "items":[{"label":"Interpretation","value":finding.interpretation},{"label":"Uncertainty","value":finding.uncertainty},{"label":"Evidence ids","value":finding.evidence.join(", ")}]}));
    }
    if chart {
        sections.extend(charts(points, &evidence)?);
    }

    Ok(
        json!({"schema":"openrows.monitor-report", "version":1, "title":title, "generatedMs":generated, "connection":{"name":"Selected connection","engine":"See source evidence","database":"Selected database"}, "range":{"fromMs":from,"toMs":to,"label":"Authorised evidence"}, "sections":sections}),
    )
}

fn generated_ms() -> Result<f64, AgentError> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AgentError::Malformed)?
        .as_secs_f64()
        * 1000.0)
}

fn charts(
    points: &[Evidence],
    evidence: &serde_json::Value,
) -> Result<Vec<serde_json::Value>, AgentError> {
    let mut groups: BTreeMap<&str, BTreeMap<&str, Vec<&Evidence>>> = BTreeMap::new();
    for point in points {
        groups
            .entry(&point.metric)
            .or_default()
            .entry(&point.scope)
            .or_default()
            .push(point);
    }
    if groups.len() > 24 {
        return Err(AgentError::Refused);
    }
    Ok(groups.into_iter().enumerate().map(|(index,(metric, scopes))| {
        let series: Vec<_> = scopes.into_iter().map(|(scope, mut points)| {
            points.sort_by(|left,right| left.collected_ms.total_cmp(&right.collected_ms));
            json!({"name":scope,"points":points.iter().map(|p|json!({"x":p.collected_ms,"label":p.collected_ms.to_string(),"y":p.value,"state":serde_json::to_value(p.state).unwrap_or_default()})).collect::<Vec<_>>()})
        }).collect();
        json!({"id":format!("chart-{index}"),"title":metric,"kind":"chart","evidence":evidence,"unit":"source unit","mark":"scatter","xKind":"time","cadenceMs":null,"series":series})
    }).collect())
}
