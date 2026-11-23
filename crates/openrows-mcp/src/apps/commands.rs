// SPDX-License-Identifier: Apache-2.0
//! Each tool's own MCP commands, verified against the installed CLIs'
//! `mcp add/get/list/remove --help` (Claude Code 2.1, codex-cli 0.171,
//! Gemini CLI 0.68). Registration is always user-wide and stdio; the token is
//! never part of the command: the bridge reads it from the keychain.
use super::{AgentApp, SERVER_NAME};
use std::path::PathBuf;

/// What the registered entry launches: the bridge, the app's port and the client id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BridgeTarget {
    /// Absolute `openrows-mcp` path.
    pub bridge: PathBuf,
    /// Fixed loopback port of the built-in server.
    pub port: u16,
    /// Client id naming the keychain item.
    pub client: String,
}
impl BridgeTarget {
    /// The bridge's own arguments.
    #[must_use]
    pub fn args(&self) -> Vec<String> {
        vec![
            "--port".into(),
            self.port.to_string(),
            "--client".into(),
            self.client.clone(),
        ]
    }
    /// The launch line as tools report it: command then arguments.
    #[must_use]
    pub fn line(&self) -> String {
        let mut words = vec![self.bridge.to_string_lossy().into_owned()];
        words.extend(self.args());
        words.join(" ")
    }
}

/// `mcp add` arguments (after the tool's executable).
#[must_use]
pub fn add_args(app: AgentApp, target: &BridgeTarget) -> Vec<String> {
    let mut args: Vec<String> = match app {
        // claude mcp add [options] <name> <commandOrUrl> [args...]
        AgentApp::Claude => vec!["mcp", "add", "--scope", "user", SERVER_NAME, "--"],
        // codex mcp add <NAME> -- <COMMAND>...
        AgentApp::Codex => vec!["mcp", "add", SERVER_NAME, "--"],
        // gemini mcp add [options] <name> <commandOrUrl> [args...]; args after `--`
        AgentApp::Gemini => vec!["mcp", "add", "--scope", "user", SERVER_NAME],
    }
    .into_iter()
    .map(str::to_owned)
    .collect();
    args.push(target.bridge.to_string_lossy().into_owned());
    if app == AgentApp::Gemini {
        args.push("--".into());
    }
    args.extend(target.args());
    args
}
/// `mcp remove` arguments for the user-wide entry.
#[must_use]
pub fn remove_args(app: AgentApp) -> Vec<String> {
    match app {
        AgentApp::Claude | AgentApp::Gemini => {
            vec!["mcp", "remove", "--scope", "user", SERVER_NAME]
        }
        AgentApp::Codex => vec!["mcp", "remove", SERVER_NAME],
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}
/// Read-only check of the entry. Claude and Gemini health-check it (which
/// starts the bridge); Codex prints its stored configuration.
#[must_use]
pub fn check_args(app: AgentApp) -> Vec<String> {
    match app {
        AgentApp::Claude => vec!["mcp", "get", SERVER_NAME],
        AgentApp::Codex => vec!["mcp", "get", "--json", SERVER_NAME],
        AgentApp::Gemini => vec!["mcp", "list"],
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}
/// A copyable shell rendering of `program args…`; execution never uses a shell.
#[must_use]
pub fn display(program: &str, args: &[String]) -> String {
    std::iter::once(program)
        .chain(args.iter().map(String::as_str))
        .map(quote)
        .collect::<Vec<_>>()
        .join(" ")
}
fn quote(word: &str) -> String {
    let safe = !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_./:=@+,".contains(&byte));
    if safe {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> BridgeTarget {
        BridgeTarget {
            bridge: PathBuf::from("/Applications/OpenRows.app/Contents/MacOS/openrows-mcp"),
            port: 32123,
            client: "claude-0a1b".into(),
        }
    }

    #[test]
    fn add_commands_match_each_tools_syntax() {
        let bridge = "/Applications/OpenRows.app/Contents/MacOS/openrows-mcp";
        assert_eq!(
            display("claude", &add_args(AgentApp::Claude, &target())),
            format!(
                "claude mcp add --scope user openrows -- {bridge} --port 32123 --client claude-0a1b"
            )
        );
        assert_eq!(
            display("codex", &add_args(AgentApp::Codex, &target())),
            format!("codex mcp add openrows -- {bridge} --port 32123 --client claude-0a1b")
        );
        assert_eq!(
            display("gemini", &add_args(AgentApp::Gemini, &target())),
            format!(
                "gemini mcp add --scope user openrows {bridge} -- --port 32123 --client claude-0a1b"
            )
        );
    }

    #[test]
    fn remove_and_check_commands_are_user_wide() {
        assert_eq!(
            remove_args(AgentApp::Claude).join(" "),
            "mcp remove --scope user openrows"
        );
        assert_eq!(
            remove_args(AgentApp::Codex).join(" "),
            "mcp remove openrows"
        );
        assert_eq!(
            remove_args(AgentApp::Gemini).join(" "),
            "mcp remove --scope user openrows"
        );
        assert_eq!(
            check_args(AgentApp::Codex).join(" "),
            "mcp get --json openrows"
        );
        assert_eq!(check_args(AgentApp::Gemini).join(" "), "mcp list");
    }

    #[test]
    fn display_quotes_only_when_needed_and_holds_no_token() {
        let mut spaced = target();
        spaced.bridge = PathBuf::from("/Users/a b/openrows-mcp");
        let text = display("claude", &add_args(AgentApp::Claude, &spaced));
        assert!(text.contains("'/Users/a b/openrows-mcp'"));
        assert_eq!(quote("it's"), r"'it'\''s'");
        assert!(!text.contains("TOKEN"));
        assert_eq!(
            target().line(),
            "/Applications/OpenRows.app/Contents/MacOS/openrows-mcp --port 32123 --client claude-0a1b"
        );
    }
}
