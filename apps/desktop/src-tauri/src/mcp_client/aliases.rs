// SPDX-License-Identifier: Apache-2.0
//! Short, safe provider-facing aliases for enabled external tools, and the
//! `server → tool` names the user sees instead of them.
use super::state::Inner;
use openrows_mcp::client::{ClientTool, ServerConfig};

/// A provider-facing alias with the server and tool it stands for.
pub(super) type AliasedTool = (String, ServerConfig, ClientTool);

/// Enabled tools in server order (user servers, then trusted Project servers)
/// as `user_<server>_<tool>`, at most 64.
pub(super) fn enabled_tools(inner: &Inner) -> Vec<AliasedTool> {
    inner
        .servers()
        .into_iter()
        .enumerate()
        .flat_map(|(server, config)| {
            let enabled = config.enabled_tools.clone();
            inner
                .tools
                .get(&config.id)
                .into_iter()
                .flatten()
                .enumerate()
                .filter(move |(_, tool)| enabled.contains(&tool.name))
                .map(move |(index, tool)| {
                    (
                        format!("user_{server}_{index}"),
                        config.clone(),
                        tool.clone(),
                    )
                })
        })
        .take(64)
        .collect()
}

/// `server → tool` for an external alias such as `user_0_0`; built-ins keep their name.
pub(super) fn tool_label(tools: &[AliasedTool], name: &str) -> Option<String> {
    tools
        .iter()
        .find(|(alias, _, _)| alias == name)
        .map(|(_, server, tool)| format!("{} → {}", server.name, tool.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_mcp::client::Endpoint;

    #[test]
    fn traces_name_external_tools_by_server_and_tool_but_keep_the_provider_alias() {
        let server = ServerConfig {
            id: "docs".into(),
            name: "Team docs".into(),
            endpoint: Endpoint::Http {
                url: "https://mcp.example.test/".into(),
            },
            enabled_tools: vec!["search_pages".into()],
        };
        let tool = ClientTool {
            name: "search_pages".into(),
            schema: serde_json::json!({"type":"object"}),
        };
        let tools = vec![("user_0_0".to_owned(), server, tool)];
        assert_eq!(
            tool_label(&tools, "user_0_0").as_deref(),
            Some("Team docs → search_pages")
        );
        assert_eq!(tool_label(&tools, "run_read_query"), None);
        assert_eq!(tools[0].0, "user_0_0");
    }
}
