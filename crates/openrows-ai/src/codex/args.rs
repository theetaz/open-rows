// SPDX-License-Identifier: Apache-2.0
//! The most restrictive app-server configuration this bridge passes on every launch.
//!
//! Unknown feature names are ignored by Codex, so later releases that drop a
//! feature still start; a release that adds a new built-in tool is covered by
//! the read-only sandbox and the declined approvals.

/// Configuration overrides (`-c key=value`, values parsed as TOML).
const OVERRIDES: &[&str] = &[
    "approval_policy=\"on-request\"",
    "sandbox_mode=\"read-only\"",
    "web_search=\"disabled\"",
    "cli_auth_credentials_store=\"file\"",
    "history.persistence=\"none\"",
    "project_doc_max_bytes=0",
    "shell_environment_policy.inherit=\"none\"",
    "analytics.enabled=false",
    "check_for_update_on_startup=false",
    "mcp_servers={}",
];
/// Built-in tools and integrations that could reach the shell, files, browser,
/// network, plugins or other agents.
const DISABLED_FEATURES: &[&str] = &[
    "shell_tool",
    "unified_exec",
    "shell_snapshot",
    "apps",
    "plugins",
    "remote_plugin",
    "multi_agent",
    "browser_use",
    "browser_use_external",
    "in_app_browser",
    "computer_use",
    "image_generation",
    "view_image",
    "hooks",
    "memories",
    "skill_search",
    "skill_mcp_dependency_install",
    "tool_suggest",
    "goals",
    "code_mode_host",
    "daemon_auto_start",
    "worktrees",
    "realtime_conversation",
];
/// Notifications the bridge never reads.
pub(crate) const QUIET: &[&str] = &[
    "item/reasoning/summaryTextDelta",
    "item/reasoning/summaryPartAdded",
    "item/reasoning/textDelta",
    "thread/tokenUsage/updated",
    "account/rateLimits/updated",
    "remoteControl/status/changed",
    "turn/diff/updated",
    "turn/plan/updated",
];

/// Arguments for `codex app-server` over stdio.
pub(crate) fn app_server() -> Vec<String> {
    let mut args = vec!["app-server".to_owned()];
    let features = DISABLED_FEATURES
        .iter()
        .map(|name| format!("features.{name}=false"));
    for setting in OVERRIDES
        .iter()
        .map(|value| (*value).to_owned())
        .chain(features)
    {
        args.push("-c".into());
        args.push(setting);
    }
    args
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_launch_disables_shell_files_web_and_mcp() {
        let args = super::app_server();
        assert_eq!(args.first().map(String::as_str), Some("app-server"));
        for setting in [
            "sandbox_mode=\"read-only\"",
            "features.shell_tool=false",
            "features.unified_exec=false",
            "web_search=\"disabled\"",
            "mcp_servers={}",
        ] {
            assert!(args.iter().any(|arg| arg == setting), "{setting}");
        }
        assert!(args[1..].chunks(2).all(|pair| pair[0] == "-c"));
    }
}
