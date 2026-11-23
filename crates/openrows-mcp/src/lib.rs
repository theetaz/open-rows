// SPDX-License-Identifier: Apache-2.0
//! Tauri-free read/propose-only MCP protocol, authenticated local transports and tokens.
#![forbid(unsafe_code)]
mod activity;
pub mod apps;
mod artifacts;
mod bridge_cli;
pub mod client;
mod connections;
mod error;
mod execution;
mod executor;
mod http;
mod links;
mod proxy;
mod rows;
mod server;
mod source;
mod tokens;
mod tools;
mod types;

pub use activity::{ActivityEvent, ActivityLog, ActivityOutcome, TokenUsage};
pub use bridge_cli::{BridgeCommand, parse_args, run_bridge};
pub use error::McpError;
pub use execution::{ReadEngine, protected_read};
pub use executor::ToolExecutor;
pub use http::{McpHost, RunningServer};
pub use links::AppLink;
pub use proxy::stdio_bridge;
pub use tokens::TokenStore;
pub use types::{
    ClientInfo, CreatedToken, McpSettings, ServerStatus, TokenInfo, TokenScope, ToolBackend,
    ToolContext, ToolInput,
};
