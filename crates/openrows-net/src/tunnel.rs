// SPDX-License-Identifier: Apache-2.0
//! Multi-hop SSH tunnel: TCP to the first hop, then each further hop over a
//! `direct-tcpip` channel of the previous one (`ProxyJump`). Destinations are
//! reached through in-process [`TunnelStream`]s; no local port is opened.

mod connect;
mod shared;
mod stream;

use std::{fmt, sync::Arc, time::Duration};

use russh::{Disconnect, client::Handle};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    time::Instant,
};

use connect::{dial, handshake};
pub use shared::SharedTunnel;
pub use stream::TunnelStream;

use crate::{
    AgentSource, AuthMethod, HostKeyPolicy, KeyboardInteractivePrompter, ProxySpec, SshError,
    Stage, Timeouts,
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
    /// SOCKS5 or HTTP proxy that carries the TCP stream to the first hop.
    pub proxy: Option<Arc<ProxySpec>>,
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
    /// Hops whose unknown host key was accepted for this connection only.
    unremembered_keys: Vec<String>,
}

impl SshTunnel {
    /// Connect and authenticate every hop in order.
    ///
    /// # Errors
    /// [`SshError`] naming the failing stage, host-key problem or auth failure.
    pub async fn connect(hops: &[SshHop], options: &TunnelOptions) -> Result<Self, SshError> {
        Self::connect_traced(hops, options).await.0
    }

    /// As [`SshTunnel::connect`], also returning the timings of the hops that
    /// came up, so diagnostics can show how far a failed attempt got (the
    /// failing hop is `hops[timings.len()]`).
    pub async fn connect_traced(
        hops: &[SshHop],
        options: &TunnelOptions,
    ) -> (Result<Self, SshError>, Vec<HopTiming>) {
        let mut tunnel = Self {
            sessions: Vec::with_capacity(hops.len()),
            timings: Vec::with_capacity(hops.len()),
            timeouts: options.timeouts,
            unremembered_keys: Vec::new(),
        };
        match tunnel.add_hops(hops, options).await {
            Ok(()) => {
                let timings = tunnel.timings.clone();
                (Ok(tunnel), timings)
            }
            Err(error) => (Err(error), std::mem::take(&mut tunnel.timings)),
        }
    }

    async fn add_hops(&mut self, hops: &[SshHop], options: &TunnelOptions) -> Result<(), SshError> {
        let (first, rest) = hops.split_first().ok_or(SshError::EmptyRoute)?;
        let started = Instant::now();
        let tcp = match &options.proxy {
            Some(proxy) => {
                proxy
                    .connect(&first.host, first.port, &options.timeouts)
                    .await?
            }
            None => dial(first, &options.timeouts).await?,
        };
        let transport = Transport {
            stream: tcp,
            connect: started.elapsed(),
        };
        self.add_hop(transport, first, options).await?;
        for hop in rest {
            let started = Instant::now();
            let TunnelStream(stream) = self.open_stream(&hop.host, hop.port).await?;
            let transport = Transport {
                stream,
                connect: started.elapsed(),
            };
            self.add_hop(transport, hop, options).await?;
        }
        Ok(())
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

    /// Open a forwarded stream to the Unix socket `path` on the last hop
    /// (`direct-streamlocal@openssh.com`).
    ///
    /// # Errors
    /// [`SshError::SocketOpen`] if the server refuses, or a timeout.
    pub async fn open_socket(&self, path: &str) -> Result<TunnelStream, SshError> {
        let session = self.sessions.last().ok_or(SshError::EmptyRoute)?;
        let open = async {
            session
                .channel_open_direct_streamlocal(path)
                .await
                .map_err(|error| SshError::SocketOpen {
                    path: path.to_owned(),
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

    /// Hosts whose unknown key was accepted for this connection only
    /// (not remembered): the route's server identity is unverified.
    #[must_use]
    pub fn unremembered_host_keys(&self) -> &[String] {
        &self.unremembered_keys
    }

    /// True once any hop's session has ended; open a new tunnel then.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.sessions.iter().any(Handle::is_closed)
    }

    /// Ping every hop (`keepalive@openssh.com`) and wait for the replies:
    /// `false` when a hop has closed or does not answer within `limit`, as a
    /// half-open transport after sleep or a network change does not.
    pub async fn probe(&self, limit: Duration) -> bool {
        let pings = async {
            for session in &self.sessions {
                session.send_ping().await?;
            }
            Ok::<_, russh::Error>(())
        };
        let answered = matches!(tokio::time::timeout(limit, pings).await, Ok(Ok(())));
        // A hop that ended drops the ping's reply channel, which also returns.
        answered && !self.is_closed()
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
        let (session, remembered) = handshake(transport.stream, hop, options).await?;
        if !remembered {
            self.unremembered_keys.push(hop.host.clone());
        }
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
