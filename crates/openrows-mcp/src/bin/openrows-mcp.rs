// SPDX-License-Identifier: Apache-2.0
//! External-agent stdio entry: explicit app port, and a keychain (`--client`)
//! or environment (`OPENROWS_MCP_TOKEN`) credential, never one in argv.
use openrows_mcp::{McpError, parse_args, run_bridge};
use openrows_secrets::KeychainSecretStore;
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), McpError> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let command = parse_args(&arguments)?;
    run_bridge(command, &KeychainSecretStore::new(), std::io::stdin()).await
}
