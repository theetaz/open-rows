// SPDX-License-Identifier: Apache-2.0
//! `openrows-mcp` arguments and credential sources.
//!
//! An agent app registered from the Apps page runs `--port P --client ID`
//! and the bridge reads its token from the OS keychain, so the app's own MCP
//! configuration holds no secret. The bridge also writes (`--store`, token on
//! stdin) and deletes (`--forget`) that item itself: the keychain then trusts
//! the bridge as the item's creator and does not prompt when it reads it.
//! `--port P` alone keeps the original `OPENROWS_MCP_TOKEN` environment path.
use crate::{McpError, stdio_bridge};
use openrows_secrets::{SecretKey, SecretStore, SecretString};
use std::io::Read;

/// What one invocation of the bridge does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BridgeCommand {
    /// Serve stdio MCP; with a client id the token comes from the keychain.
    Serve {
        /// App loopback port.
        port: u16,
        /// Registered app client id.
        client: Option<String>,
    },
    /// Save the token read from stdin for a client id.
    Store {
        /// Registered app client id.
        client: String,
    },
    /// Delete a client id's token.
    Forget {
        /// Registered app client id.
        client: String,
    },
}

/// Parse arguments after the program name; anything unexpected is refused.
/// # Errors
/// Unknown flags, a missing or zero port, or an invalid client id.
pub fn parse_args(arguments: &[String]) -> Result<BridgeCommand, McpError> {
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["--port", port] => Ok(BridgeCommand::Serve {
            port: port_of(port)?,
            client: None,
        }),
        ["--port", port, "--client", client] => Ok(BridgeCommand::Serve {
            port: port_of(port)?,
            client: Some(client_of(client)?),
        }),
        ["--client", client, "--store"] => Ok(BridgeCommand::Store {
            client: client_of(client)?,
        }),
        ["--client", client, "--forget"] => Ok(BridgeCommand::Forget {
            client: client_of(client)?,
        }),
        _ => Err(McpError::Invalid),
    }
}
fn port_of(text: &str) -> Result<u16, McpError> {
    text.parse()
        .ok()
        .filter(|port| *port != 0)
        .ok_or(McpError::Invalid)
}
fn client_of(text: &str) -> Result<String, McpError> {
    openrows_ai::validate_id(text).map_err(|_| McpError::Invalid)?;
    Ok(text.to_owned())
}

/// Run a parsed command with the given secret store and standard input.
/// # Errors
/// Missing credentials, keychain failure or the bridge's own transport errors.
pub async fn run_bridge(
    command: BridgeCommand,
    secrets: &dyn SecretStore,
    input: impl Read,
) -> Result<(), McpError> {
    match command {
        BridgeCommand::Serve { port, client } => {
            let credential = match client {
                Some(client) => keychain_token(secrets, &client)?,
                None => std::env::var("OPENROWS_MCP_TOKEN").map_err(|_| McpError::Auth)?,
            };
            stdio_bridge(port, credential).await
        }
        BridgeCommand::Store { client } => {
            let token = read_token(input)?;
            secrets
                .set(&SecretKey::mcp_client(&client), &token)
                .map_err(|_| McpError::Store)
        }
        BridgeCommand::Forget { client } => secrets
            .delete(&SecretKey::mcp_client(&client))
            .map_err(|_| McpError::Store),
    }
}
fn keychain_token(secrets: &dyn SecretStore, client: &str) -> Result<String, McpError> {
    secrets
        .get(&SecretKey::mcp_client(client))
        .map_err(|_| McpError::Store)?
        .map(|value| value.expose().to_owned())
        .ok_or(McpError::Auth)
}
/// Exactly one 64-hex token, optionally newline terminated.
fn read_token(input: impl Read) -> Result<SecretString, McpError> {
    let mut text = String::new();
    input
        .take(130)
        .read_to_string(&mut text)
        .map_err(|_| McpError::Invalid)?;
    let token = text.trim_end_matches(['\r', '\n']);
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(McpError::Invalid);
    }
    Ok(SecretString::from(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_secrets::MemorySecretStore;

    fn words(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn arguments_cover_environment_keychain_store_and_forget() {
        assert_eq!(
            parse_args(&words("--port 32123")),
            Ok(BridgeCommand::Serve {
                port: 32123,
                client: None
            })
        );
        assert_eq!(
            parse_args(&words("--port 32123 --client claude-0a1b")),
            Ok(BridgeCommand::Serve {
                port: 32123,
                client: Some("claude-0a1b".into())
            })
        );
        assert!(matches!(
            parse_args(&words("--client codex-1 --store")),
            Ok(BridgeCommand::Store { .. })
        ));
        for bad in [
            "",
            "--port 0",
            "--port x",
            "--port 1 --client ../x",
            "--client a --store extra",
            "--client a --print",
        ] {
            assert_eq!(parse_args(&words(bad)), Err(McpError::Invalid), "{bad}");
        }
    }

    #[tokio::test]
    async fn store_validates_and_forget_removes_the_keychain_item() {
        let secrets = MemorySecretStore::default();
        let token = "ab".repeat(32);
        let input = format!("{token}\n");
        let command = BridgeCommand::Store {
            client: "gemini-2".into(),
        };
        assert_eq!(
            run_bridge(command.clone(), &secrets, input.as_bytes()).await,
            Ok(())
        );
        assert_eq!(keychain_token(&secrets, "gemini-2"), Ok(token));
        assert_eq!(
            run_bridge(command, &secrets, "short".as_bytes()).await,
            Err(McpError::Invalid)
        );
        let forget = BridgeCommand::Forget {
            client: "gemini-2".into(),
        };
        assert_eq!(run_bridge(forget, &secrets, std::io::empty()).await, Ok(()));
        assert_eq!(keychain_token(&secrets, "gemini-2"), Err(McpError::Auth));
    }
}
