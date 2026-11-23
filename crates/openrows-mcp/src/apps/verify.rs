// SPDX-License-Identifier: Apache-2.0
//! Read the `openrows` entry back from each tool's own check command. Only
//! that entry's launch line and health are kept; other servers' lines (which
//! may hold their arguments) are discarded and never logged or returned.
use super::{AgentApp, AppError, Output, SERVER_NAME};
use serde::Serialize;

/// What the tool reports for the `openrows` entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    /// The entry exists in the tool's configuration.
    pub present: bool,
    /// Its launch line, command then arguments separated by spaces.
    pub line: Option<String>,
    /// The tool's own health check, when it ran one.
    pub healthy: Option<bool>,
}
impl Registration {
    fn found(line: String, healthy: Option<bool>) -> Self {
        Self {
            present: true,
            line: Some(line),
            healthy,
        }
    }
}

/// Interpret one tool's check output.
/// # Errors
/// Output that matches neither a found nor a missing entry.
pub fn parse_check(app: AgentApp, output: &Output) -> Result<Registration, AppError> {
    let missing = output.text.contains("No MCP server named");
    match app {
        AgentApp::Claude | AgentApp::Codex if missing => Ok(Registration::default()),
        AgentApp::Claude => claude(&output.text),
        AgentApp::Codex => codex(&output.text),
        AgentApp::Gemini => gemini(&output.text),
    }
}
/// `  Status: ✓ Connected` / `  Command: …` / `  Args: …` blocks.
fn claude(text: &str) -> Result<Registration, AppError> {
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(name).map(str::trim))
    };
    let command = field("Command:").ok_or(AppError::Unrecognised)?;
    let args = field("Args:").unwrap_or_default();
    let healthy = field("Status:").map(|status| status.contains("Connected"));
    Ok(Registration::found(
        format!("{command} {args}").trim().to_owned(),
        healthy,
    ))
}
/// `codex mcp get --json` prints the stored entry.
fn codex(text: &str) -> Result<Registration, AppError> {
    let start = text.find('{').ok_or(AppError::Unrecognised)?;
    let value: serde_json::Value = serde_json::Deserializer::from_str(&text[start..])
        .into_iter()
        .next()
        .and_then(Result::ok)
        .ok_or(AppError::Unrecognised)?;
    let transport = &value["transport"];
    let command = transport["command"]
        .as_str()
        .ok_or(AppError::Unrecognised)?;
    let mut words = vec![command.to_owned()];
    words.extend(
        transport["args"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|arg| arg.as_str().map(str::to_owned)),
    );
    let enabled = value["enabled"].as_bool();
    Ok(Registration::found(
        words.join(" "),
        enabled.filter(|on| !on),
    ))
}
/// `✓ openrows: <command> <args> (stdio) - Connected` lines.
fn gemini(text: &str) -> Result<Registration, AppError> {
    let prefix = format!("{SERVER_NAME}: ");
    for line in text.lines() {
        let line = line.trim();
        let entry = line.strip_prefix(prefix.as_str()).or_else(|| {
            line.split_once(' ')
                .and_then(|(_, rest)| rest.strip_prefix(prefix.as_str()))
        });
        let Some(entry) = entry else { continue };
        let (launch, status) = entry.rsplit_once(" - ").unwrap_or((entry, ""));
        let launch = launch.trim_end_matches(" (stdio)").trim();
        let healthy = match status.trim() {
            "Connected" => Some(true),
            "" | "Connecting" => None,
            _ => Some(false),
        };
        return Ok(Registration::found(launch.to_owned(), healthy));
    }
    if text.contains("Configured MCP servers") || text.contains("No MCP servers configured") {
        return Ok(Registration::default());
    }
    Err(AppError::Unrecognised)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(text: &str) -> Output {
        Output {
            success: true,
            text: text.into(),
        }
    }

    #[test]
    fn claude_reads_its_get_block() {
        let text = "openrows:\n  Scope: User config (available in all your projects)\n  Status: ✓ Connected\n  Type: stdio\n  Command: /x/openrows-mcp\n  Args: --port 1 --client c\n  Environment:\n\nTo remove this server, run: claude mcp remove \"openrows\" -s user\n";
        assert_eq!(
            parse_check(AgentApp::Claude, &output(text)),
            Ok(Registration::found(
                "/x/openrows-mcp --port 1 --client c".into(),
                Some(true)
            ))
        );
        let missing = "No MCP server named \"openrows\". Configured servers: a, b";
        assert_eq!(
            parse_check(AgentApp::Claude, &output(missing)),
            Ok(Registration::default())
        );
        assert_eq!(
            parse_check(AgentApp::Claude, &output("unexpected")),
            Err(AppError::Unrecognised)
        );
    }

    #[test]
    fn codex_reads_its_json_entry() {
        let text = r#"{"name":"openrows","enabled":true,"transport":{"type":"stdio","command":"/x/openrows-mcp","args":["--port","1","--client","c"],"env":null}}"#;
        assert_eq!(
            parse_check(AgentApp::Codex, &output(text)),
            Ok(Registration::found(
                "/x/openrows-mcp --port 1 --client c".into(),
                None
            ))
        );
        let disabled = text.replace("\"enabled\":true", "\"enabled\":false");
        assert_eq!(
            parse_check(AgentApp::Codex, &output(&disabled)).map(|entry| entry.healthy),
            Ok(Some(false))
        );
        let missing = "Error: No MCP server named 'openrows' found.";
        assert_eq!(
            parse_check(AgentApp::Codex, &output(missing)),
            Ok(Registration::default())
        );
    }

    #[test]
    fn gemini_reads_only_its_own_list_line() {
        let text = "Configured MCP servers:\n\n✓ other: /bin/other --secret-flag (stdio) - Connected\n✗ openrows: /x/openrows-mcp --port 1 --client c (stdio) - Disconnected\n";
        assert_eq!(
            parse_check(AgentApp::Gemini, &output(text)),
            Ok(Registration::found(
                "/x/openrows-mcp --port 1 --client c".into(),
                Some(false)
            ))
        );
        let other = "Configured MCP servers:\n\n✓ openrows-extra: /bin/x (stdio) - Connected\n";
        assert_eq!(
            parse_check(AgentApp::Gemini, &output(other)),
            Ok(Registration::default())
        );
        assert_eq!(
            parse_check(AgentApp::Gemini, &output("No MCP servers configured.")),
            Ok(Registration::default())
        );
    }
}
