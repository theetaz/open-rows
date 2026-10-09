// SPDX-License-Identifier: Apache-2.0
//! Per-hop connection set-up: DNS + TCP for the first hop, then the SSH
//! handshake and authentication within their stage budgets.

use std::{
    borrow::Cow,
    io,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use russh::{
    Preferred,
    client::{self, Handle},
    keys::Algorithm,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, lookup_host},
};

use super::{SshHop, TunnelOptions};
use crate::{
    KeyboardInteractivePrompter, SshError, Stage, Timeouts,
    auth::{AuthContext, authenticate},
    blocking::off_runtime,
    handler::ClientHandler,
    timeouts::{PromptClock, within},
};

/// Resolve and TCP-connect the first hop (addresses tried in order;
/// Happy Eyeballs arrives with the M1-06 route builder).
pub(super) async fn dial(hop: &SshHop, timeouts: &Timeouts) -> Result<TcpStream, SshError> {
    let clock = PromptClock::default();
    let resolve = async {
        let addresses: Vec<SocketAddr> = lookup_host((hop.host.as_str(), hop.port))
            .await
            .map_err(|source| resolve_error(hop, source))?
            .collect();
        if addresses.is_empty() {
            return Err(resolve_error(hop, io::ErrorKind::NotFound.into()));
        }
        Ok(addresses)
    };
    let addresses = within(Stage::Resolve, timeouts.dns, &clock, resolve).await?;
    let connect = async {
        let mut last_error = io::Error::from(io::ErrorKind::NotFound);
        for address in &addresses {
            match TcpStream::connect(address).await {
                Ok(stream) => return Ok(stream),
                Err(error) => last_error = error,
            }
        }
        Err(SshError::Connect {
            host: hop.host.clone(),
            port: hop.port,
            source: last_error,
        })
    };
    let stream = within(Stage::TcpConnect, timeouts.tcp_connect, &clock, connect).await?;
    // Latency matters more than packet count for interactive database traffic.
    stream
        .set_nodelay(true)
        .map_err(|source| SshError::Connect {
            host: hop.host.clone(),
            port: hop.port,
            source,
        })?;
    Ok(stream)
}

fn resolve_error(hop: &SshHop, source: io::Error) -> SshError {
    SshError::Resolve {
        host: hop.host.clone(),
        source,
    }
}

/// SSH handshake + authentication over `stream`, within the handshake
/// budget. The flag is false when an unknown host key was accepted once.
pub(super) async fn handshake<S>(
    stream: S,
    hop: &SshHop,
    options: &TunnelOptions,
) -> Result<(Handle<ClientHandler>, bool), SshError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let clock = PromptClock::default();
    let config = Arc::new(client_config(hop, options).await?);
    let accepted_once = Arc::new(AtomicBool::new(false));
    let handler = ClientHandler {
        host: hop.host.clone(),
        port: hop.port,
        known_hosts: Arc::clone(&options.known_hosts),
        policy: Arc::clone(&options.host_key_policy),
        clock: clock.clone(),
        accepted_once: Arc::clone(&accepted_once),
    };
    let work = async {
        let mut session = client::connect_stream(config, stream, handler).await?;
        let context = AuthContext {
            host: &hop.host,
            user: &hop.user,
            prompter: options.prompter.as_ref() as &dyn KeyboardInteractivePrompter,
            clock: &clock,
        };
        authenticate(&mut session, &hop.auth, &context).await?;
        Ok(session)
    };
    let session = within(Stage::Handshake, options.timeouts.handshake, &clock, work).await?;
    Ok((session, !accepted_once.load(Ordering::Relaxed)))
}

async fn client_config(hop: &SshHop, options: &TunnelOptions) -> Result<client::Config, SshError> {
    let (store, host, port) = (Arc::clone(&options.known_hosts), hop.host.clone(), hop.port);
    let known = off_runtime(move || store.known_algorithms(&host, port)).await?;
    Ok(client::Config {
        keepalive_interval: Some(options.timeouts.keepalive_interval),
        nodelay: true,
        preferred: Preferred {
            key: Cow::Owned(host_key_order(&known)),
            ..Preferred::default()
        },
        ..client::Config::default()
    })
}

/// Verifiable host-key algorithms, already-known ones first (as OpenSSH does).
/// RSA host keys are excluded: verifying them needs the `rsa` crate.
pub(super) fn host_key_order(known: &[Algorithm]) -> Vec<Algorithm> {
    let (mut order, rest): (Vec<Algorithm>, Vec<Algorithm>) = Preferred::DEFAULT
        .key
        .iter()
        .filter(|algorithm| !matches!(algorithm, Algorithm::Rsa { .. }))
        .cloned()
        .partition(|algorithm| known.contains(algorithm));
    order.extend(rest);
    order
}
