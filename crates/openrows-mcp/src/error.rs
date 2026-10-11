// SPDX-License-Identifier: Apache-2.0
//! Fixed errors never carry database contents, SQL, credentials or URLs.
/// MCP operation refusal safe to send to an untrusted client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum McpError {
    /// Invalid, expired or revoked credential.
    #[error("MCP access is invalid, expired or revoked")]
    Auth,
    /// Work cancelled or permission revoked.
    #[error("MCP work was cancelled or permission was revoked")]
    Cancelled,
    /// Unknown or invalid input.
    #[error("Invalid MCP request")]
    Invalid,
    /// Connection sharing policy refused context.
    #[error("Connection sharing policy refused this request")]
    Sharing,
    /// Read-only proof or engine protection unavailable.
    #[error("This query or engine cannot provide protected AI reads")]
    Unsafe,
    /// Interactive authentication is required.
    #[error("This connection requires user review in the editor")]
    Review,
    /// Resource or time budget reached.
    #[error("MCP resource limit or deadline reached")]
    Limit,
    /// Local storage failed.
    #[error("MCP local storage is unavailable")]
    Store,
    /// Driver failure, details withheld.
    #[error("The database operation failed; review it in OpenRows")]
    Database,
    /// Local transport unavailable.
    #[error("MCP local transport is unavailable")]
    Transport,
}
