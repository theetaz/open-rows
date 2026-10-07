// SPDX-License-Identifier: Apache-2.0
//! SSH tunnel prototype for `OpenRows` network routes (M0-S4 spike).
//!
//! Proves `russh` for the SSH hop of docs/24 §2–§3: password, key file,
//! ssh-agent and keyboard-interactive authentication, multi-hop `ProxyJump`
//! chains, a `~/.ssh/config` subset and `known_hosts` verification with a
//! first-use decision. Tunnels yield in-process [`TunnelStream`]s; no local
//! listener is opened (docs/12 §4.2).

#![forbid(unsafe_code)]

mod auth;
mod blocking;
pub mod config;
mod error;
mod handler;
pub mod known_hosts;
mod route;
mod secret;
mod timeouts;
mod tunnel;

use std::{future::Future, pin::Pin};

pub use auth::{
    AgentSource, AuthMethod, Challenge, ChallengePrompt, KeyboardInteractivePrompter,
    PromptCancelled,
};
pub use error::SshError;
pub use handler::{FirstUseDecision, HostKeyPolicy, UnknownHost};
pub use route::{RouteError, resolve_route};
pub use secret::SecretString;
pub use timeouts::{Stage, Timeouts};
pub use tunnel::{HopTiming, SshHop, SshTunnel, TunnelOptions, TunnelStream};

/// Boxed `Send` future returned by the UI-facing callback traits.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
