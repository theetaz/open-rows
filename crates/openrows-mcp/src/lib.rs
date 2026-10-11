// SPDX-License-Identifier: Apache-2.0
//! Tauri-free read/propose-only MCP protocol, authenticated local transports and tokens.
#![forbid(unsafe_code)]
mod connections;
mod error;
mod execution;
mod http;
mod proxy;
mod rows;
mod server;
mod source;
mod tokens;
mod tools;
mod types;

pub use error::McpError;
pub use execution::{ReadEngine, protected_read};
pub use http::{McpHost, RunningServer};
pub use proxy::stdio_bridge;
pub use tokens::TokenStore;
pub use types::{
    ClientInfo, CreatedToken, McpSettings, ServerStatus, TokenInfo, TokenScope, ToolBackend,
    ToolContext, ToolInput,
};
