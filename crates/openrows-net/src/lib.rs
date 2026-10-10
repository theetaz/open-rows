// SPDX-License-Identifier: Apache-2.0
//! Network routes for `OpenRows` database connections (docs/24 §1–§4).
//!
//! Proxies (M1-06): SOCKS5 and HTTP `CONNECT` as the first hop, in front of
//! the database ([`ProxyDialer`]) or of the first SSH hop
//! ([`TunnelOptions::proxy`]).
//!
//! SSH (from the M0-S4 spike, promoted for D-10): `russh` with password, key
//! file, ssh-agent and keyboard-interactive authentication, multi-hop
//! `ProxyJump` chains, a `~/.ssh/config` subset and `known_hosts`
//! verification with a first-use decision. [`plan_route`] turns a profile's
//! SSH settings into hops; tunnels yield in-process [`TunnelStream`]s that a
//! [`TunnelDialer`] hands to drivers, so no local listener is opened
//! (docs/12 §4.2). A profile's [`SharedTunnel`] is reopened when its
//! transport drops; destinations may be TCP or a Unix socket on the server.
//!
//! Resilience (M1-08): every TCP socket gets OS keepalive
//! ([`configure_tcp`]), tunnels answer an on-demand [`SharedTunnel::probe`],
//! and [`watch_path`] reports sleep/wake and network changes so callers can
//! probe their sessions at once.
//!
//! TLS (D-11): [`tls::TlsClient`] runs a rustls handshake over any stream
//! with libpq-style verification levels, custom CA files, bundled cloud
//! provider CAs and client certificates; drivers keep their
//! protocol-specific negotiation.

#![forbid(unsafe_code)]

mod auth;
mod blocking;
pub mod config;
mod direct;
mod error;
mod handler;
pub mod helper;
pub mod known_hosts;
mod monitor;
mod plan;
mod proxy;
mod route;
mod secret;
mod tcp;
#[cfg(any(test, feature = "test-proxy"))]
pub mod testing;
mod timeouts;
pub mod tls;
mod transport;
mod tunnel;

use std::{future::Future, pin::Pin};

pub use auth::{
    AgentSource, AuthMethod, Challenge, ChallengePrompt, KeyboardInteractivePrompter,
    PromptCancelled,
};
pub use direct::TcpDialer;
pub use error::SshError;
pub use handler::{FirstUseDecision, HostKeyPolicy, UnknownHost};
pub use monitor::{PathChange, PathSample, watch_path};
pub use plan::{PlanContext, SshAuth, SshRouteSpec, plan_route};
pub use proxy::{ProxyAuth, ProxyDialer, ProxyError, ProxyKind, ProxySpec};
pub use route::{RouteError, resolve_route};
pub use secret::SecretString;
pub use tcp::configure_tcp;
pub use timeouts::{Stage, Timeouts};
pub use transport::{BoxStream, Destination, Dialer, NetStream, TunnelDialer};
pub use tunnel::{HopTiming, SharedTunnel, SshHop, SshTunnel, TunnelOptions, TunnelStream};

/// Boxed `Send` future returned by the UI-facing callback traits.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
