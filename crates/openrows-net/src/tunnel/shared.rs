// SPDX-License-Identifier: Apache-2.0
//! One tunnel shared by every session of a profile (docs/24 §3), reopened
//! transparently when its SSH transport has dropped.

use std::{
    fmt,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::sync::Mutex as AsyncMutex;

use super::{SshHop, SshTunnel, TunnelOptions};
use crate::{HopTiming, SshError};

/// A profile's tunnel: the planned hops, the options to reconnect with, and
/// the current connection (if any).
///
/// Concurrent callers wait for one connection attempt, so prompts (host key,
/// MFA) appear once. A tunnel whose transport has closed is replaced on the
/// next [`SharedTunnel::tunnel`] call.
pub struct SharedTunnel {
    hops: Vec<SshHop>,
    options: TunnelOptions,
    current: Mutex<Option<Arc<SshTunnel>>>,
    connecting: AsyncMutex<()>,
}

impl SharedTunnel {
    /// Not yet connected; the first [`SharedTunnel::tunnel`] call connects.
    #[must_use]
    pub fn new(hops: Vec<SshHop>, options: TunnelOptions) -> Self {
        Self {
            hops,
            options,
            current: Mutex::new(None),
            connecting: AsyncMutex::new(()),
        }
    }

    /// Already connected through `tunnel` (e.g. by a traced connect).
    #[must_use]
    pub fn with_tunnel(tunnel: SshTunnel, hops: Vec<SshHop>, options: TunnelOptions) -> Self {
        let shared = Self::new(hops, options);
        *shared.lock() = Some(Arc::new(tunnel));
        shared
    }

    /// The live tunnel, connecting (again) when there is none or it closed.
    ///
    /// # Errors
    /// The [`SshError`] of the connection attempt.
    pub async fn tunnel(&self) -> Result<Arc<SshTunnel>, SshError> {
        if let Some(live) = self.live() {
            return Ok(live);
        }
        let _connecting = self.connecting.lock().await;
        // Another caller may have connected while this one waited.
        if let Some(live) = self.live() {
            return Ok(live);
        }
        // Boxed: the handshake state machine is large.
        let tunnel = Arc::new(Box::pin(SshTunnel::connect(&self.hops, &self.options)).await?);
        *self.lock() = Some(Arc::clone(&tunnel));
        Ok(tunnel)
    }

    /// Stop using `stale` (its transport stopped answering); the next
    /// [`SharedTunnel::tunnel`] call reconnects. A newer tunnel is kept.
    pub fn discard(&self, stale: &Arc<SshTunnel>) {
        let mut current = self.lock();
        if current
            .as_ref()
            .is_some_and(|tunnel| Arc::ptr_eq(tunnel, stale))
        {
            *current = None;
        }
    }

    /// The current tunnel, open or not, without connecting.
    #[must_use]
    pub fn current(&self) -> Option<Arc<SshTunnel>> {
        self.lock().clone()
    }

    /// Hosts of the planned route, first hop first.
    #[must_use]
    pub fn hosts(&self) -> Vec<&str> {
        self.hops.iter().map(|hop| hop.host.as_str()).collect()
    }

    /// Per-hop timings of the current tunnel; empty before it connected.
    #[must_use]
    pub fn timings(&self) -> Vec<HopTiming> {
        self.current()
            .map(|tunnel| tunnel.timings().to_vec())
            .unwrap_or_default()
    }

    /// Disconnect the current tunnel when nothing else uses it.
    pub async fn close(&self) {
        let taken = self.lock().take();
        if let Some(tunnel) = taken.and_then(|tunnel| Arc::try_unwrap(tunnel).ok()) {
            // Best effort: a session that already ended has nothing to say goodbye to.
            let _ = tunnel.close().await;
        }
    }

    fn live(&self) -> Option<Arc<SshTunnel>> {
        self.current().filter(|tunnel| !tunnel.is_closed())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Arc<SshTunnel>>> {
        self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl fmt::Debug for SharedTunnel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedTunnel")
            .field("hosts", &self.hosts())
            .field("connected", &self.live().is_some())
            .finish_non_exhaustive()
    }
}
