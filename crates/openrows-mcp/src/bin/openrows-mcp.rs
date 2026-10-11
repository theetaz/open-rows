// SPDX-License-Identifier: Apache-2.0
//! External-agent stdio entry; explicit app port and environment-only credential.
use openrows_mcp::{McpError, stdio_bridge};
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), McpError> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 2 || arguments[0] != "--port" {
        return Err(McpError::Invalid);
    }
    let port = arguments[1].parse().map_err(|_| McpError::Invalid)?;
    let credential = std::env::var("OPENROWS_MCP_TOKEN").map_err(|_| McpError::Auth)?;
    stdio_bridge(port, credential).await
}
