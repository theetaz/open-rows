// SPDX-License-Identifier: Apache-2.0
//! Network routes for `OpenRows` database connections (docs/24 §1–§4).
//!
//! SSH (from the M0-S4 spike, promoted for D-10): `russh` with password, key
//! file, ssh-agent and keyboard-interactive authentication, multi-hop
//! `ProxyJump` chains, a `~/.ssh/config` subset and `known_hosts`
//! verification with a first-use decision. [`plan_route`] turns a profile's
//! SSH settings into hops; tunnels yield in-process [`TunnelStream`]s that a
//! [`TunnelDialer`] hands to drivers, so no local listener is opened
//! (docs/12 §4.2).
//!
//! TLS (D-11): [`tls::TlsClient`] runs a rustls handshake over any stream
//! with libpq-style verification levels, custom CA files and client
//! certificates; drivers keep their protocol-specific negotiation.

#![forbid(unsafe_code)]

mod auth;
mod blocking;
pub mod config;
mod error;
mod handler;
pub mod known_hosts;
mod plan;
mod route;
mod secret;
mod timeouts;
pub mod tls;
mod transport;
mod tunnel;

use std::{future::Future, pin::Pin};

pub use auth::{
    AgentSource, AuthMethod, Challenge, ChallengePrompt, KeyboardInteractivePrompter,
    PromptCancelled,
};
pub use error::SshError;
pub use handler::{FirstUseDecision, HostKeyPolicy, UnknownHost};
pub use plan::{PlanContext, SshAuth, SshRouteSpec, plan_route};
pub use route::{RouteError, resolve_route};
pub use secret::SecretString;
pub use timeouts::{Stage, Timeouts};
pub use transport::{BoxStream, Dialer, NetStream, TunnelDialer};
pub use tunnel::{HopTiming, SshHop, SshTunnel, TunnelOptions, TunnelStream};

/// Boxed `Send` future returned by the UI-facing callback traits.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
