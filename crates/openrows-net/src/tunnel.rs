// SPDX-License-Identifier: Apache-2.0
//! Multi-hop SSH tunnel: TCP to the first hop, then each further hop over a
//! `direct-tcpip` channel of the previous one (`ProxyJump`). Destinations are
//! reached through in-process [`TunnelStream`]s; no local port is opened.

mod connect;
mod stream;

use std::{fmt, sync::Arc, time::Duration};

use russh::{Disconnect, client::Handle};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::Instant,
};

use connect::{dial, handshake};
pub use stream::TunnelStream;

use crate::{
    AgentSource, AuthMethod, HostKeyPolicy, KeyboardInteractivePrompter, SshError, Stage, Timeouts,
    config::HostSettings,
    handler::ClientHandler,
    known_hosts::KnownHosts,
    timeouts::{PromptClock, within},
};

/// One SSH server on the route.
#[derive(Clone, Debug)]
pub struct SshHop {
    /// Host to dial (first hop) or to reach from the previous hop.
    pub host: String,
    /// SSH port.
    pub port: u16,
    /// Login user.
    pub user: String,
    /// Methods to try, in order.
    pub auth: Vec<AuthMethod>,
}

impl SshHop {
    /// A hop for resolved config `settings`, with OpenSSH's default method
    /// order: ssh-agent (unless `IdentitiesOnly`), each `IdentityFile`, then
    /// keyboard-interactive. `default_user` applies when no `User` is set.
    #[must_use]
    pub fn from_settings(settings: &HostSettings, default_user: &str) -> Self {
        let agent =
            (!settings.identities_only).then_some(AuthMethod::Agent(AgentSource::Environment));
        let keys = settings
            .identity_files
            .iter()
            .map(|path| AuthMethod::PrivateKey {
                path: path.clone(),
                passphrase: None,
            });
        Self {
            host: settings.host_name.clone(),
            port: settings.port,
            user: settings
                .user
                .clone()
                .unwrap_or_else(|| default_user.to_owned()),
            auth: agent
                .into_iter()
                .chain(keys)
                .chain([AuthMethod::KeyboardInteractive])
                .collect(),
        }
    }
}

/// Shared services for opening a tunnel.
#[derive(Clone)]
pub struct TunnelOptions {
    /// Host-key store.
    pub known_hosts: Arc<KnownHosts>,
    /// First-use decision for unknown host keys.
    pub host_key_policy: Arc<dyn HostKeyPolicy>,
    /// Answers keyboard-interactive challenges.
    pub prompter: Arc<dyn KeyboardInteractivePrompter>,
    /// Stage budgets.
    pub timeouts: Timeouts,
}

/// How long each hop took to reach and to authenticate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HopTiming {
    /// Hop host.
    pub host: String,
    /// DNS + TCP connect (first hop) or channel open through the previous hop.
    pub connect: Duration,
    /// SSH handshake + authentication, including any prompt time.
    pub handshake: Duration,
}

/// An authenticated chain of SSH sessions.
pub struct SshTunnel {
    /// Sessions in hop order; earlier ones carry the later ones' transport.
    sessions: Vec<Handle<ClientHandler>>,
    timings: Vec<HopTiming>,
    timeouts: Timeouts,
}

impl SshTunnel {
    /// Connect and authenticate every hop in order.
    ///
    /// # Errors
    /// [`SshError`] naming the failing stage, host-key problem or auth failure.
    pub async fn connect(hops: &[SshHop], options: &TunnelOptions) -> Result<Self, SshError> {
        let (first, rest) = hops.split_first().ok_or(SshError::EmptyRoute)?;
        let mut tunnel = Self {
            sessions: Vec::with_capacity(hops.len()),
            timings: Vec::with_capacity(hops.len()),
            timeouts: options.timeouts,
        };
        let started = Instant::now();
        let tcp = dial(first, &options.timeouts).await?;
        let transport = Transport {
            stream: tcp,
            connect: started.elapsed(),
        };
        tunnel.add_hop(transport, first, options).await?;
        for hop in rest {
            let started = Instant::now();
            let TunnelStream(stream) = tunnel.open_stream(&hop.host, hop.port).await?;
            let transport = Transport {
                stream,
                connect: started.elapsed(),
            };
            tunnel.add_hop(transport, hop, options).await?;
        }
        Ok(tunnel)
    }

    /// Open a forwarded stream to `host:port` as seen from the last hop.
    ///
    /// # Errors
    /// [`SshError::ChannelOpen`] if the server refuses, or a timeout.
    pub async fn open_stream(&self, host: &str, port: u16) -> Result<TunnelStream, SshError> {
        let session = self.sessions.last().ok_or(SshError::EmptyRoute)?;
        let open = async {
            session
                .channel_open_direct_tcpip(host, u32::from(port), "127.0.0.1", 0)
                .await
                .map_err(|error| SshError::ChannelOpen {
                    host: host.to_owned(),
                    port,
                    reason: error.to_string(),
                })
        };
        let channel = within(
            Stage::ChannelOpen,
            self.timeouts.channel_open,
            &PromptClock::default(),
            open,
        )
        .await?;
        Ok(TunnelStream(channel.into_stream()))
    }

    /// Per-hop timings, first hop first.
    #[must_use]
    pub fn timings(&self) -> &[HopTiming] {
        &self.timings
    }

    /// Disconnect every hop, innermost first.
    ///
    /// # Errors
    /// [`SshError::Protocol`] if a session already stopped.
    pub async fn close(self) -> Result<(), SshError> {
        for session in self.sessions.iter().rev() {
            session
                .disconnect(Disconnect::ByApplication, "", "en")
                .await?;
        }
        Ok(())
    }

    async fn add_hop<S>(
        &mut self,
        transport: Transport<S>,
        hop: &SshHop,
        options: &TunnelOptions,
    ) -> Result<(), SshError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let started = Instant::now();
        let session = handshake(transport.stream, hop, options).await?;
        self.timings.push(HopTiming {
            host: hop.host.clone(),
            connect: transport.connect,
            handshake: started.elapsed(),
        });
        self.sessions.push(session);
        Ok(())
    }
}

/// A connected byte stream to a hop and how long reaching it took.
struct Transport<S> {
    stream: S,
    connect: Duration,
}

impl fmt::Debug for SshTunnel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SshTunnel")
            .field("timings", &self.timings)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
