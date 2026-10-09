// SPDX-License-Identifier: Apache-2.0
//! Per-stage deadlines (docs/14 §4) that stop counting while a user prompt
//! (host-key decision, keyboard-interactive challenge) is open.

use std::{
    fmt,
    future::Future,
    pin::pin,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use tokio::time::Instant;

use crate::SshError;

/// Connection stage that a timeout or timing measurement refers to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    /// DNS resolution of the first hop.
    Resolve,
    /// TCP connect to the first hop.
    TcpConnect,
    /// SSH handshake plus authentication of one hop, excluding prompts.
    Handshake,
    /// `direct-tcpip` channel open through an established hop.
    ChannelOpen,
    /// SOCKS5 or HTTP `CONNECT` negotiation with a proxy.
    Proxy,
}

impl fmt::Display for Stage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Resolve => "DNS resolution",
            Self::TcpConnect => "TCP connect",
            Self::Handshake => "SSH handshake and authentication",
            Self::ChannelOpen => "tunnel channel open",
            Self::Proxy => "proxy handshake",
        })
    }
}

/// Stage budgets; defaults follow docs/14 §4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Timeouts {
    /// DNS resolution (default 5 s).
    pub dns: Duration,
    /// TCP connect (default 10 s).
    pub tcp_connect: Duration,
    /// SSH handshake + auth per hop, excluding user prompts (default 20 s).
    pub handshake: Duration,
    /// Opening a forwarded channel; a remote TCP connect (default 10 s).
    pub channel_open: Duration,
    /// SSH-level keepalive interval (default 30 s).
    pub keepalive_interval: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            dns: Duration::from_secs(5),
            tcp_connect: Duration::from_secs(10),
            handshake: Duration::from_secs(20),
            channel_open: Duration::from_secs(10),
            keepalive_interval: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Default)]
struct ClockState {
    open_prompts: u32,
    paused_since: Option<Instant>,
    paused_total: Duration,
}

/// Shared record of time spent waiting for the user.
#[derive(Clone, Debug, Default)]
pub(crate) struct PromptClock(Arc<Mutex<ClockState>>);

/// Resumes the deadline when the prompt that created it finishes.
pub(crate) struct PromptPause(PromptClock);

impl PromptClock {
    /// Stop charging time to the current stage until the guard drops.
    pub(crate) fn pause(&self) -> PromptPause {
        let mut state = self.lock();
        if state.open_prompts == 0 {
            state.paused_since = Some(Instant::now());
        }
        state.open_prompts += 1;
        PromptPause(self.clone())
    }

    fn excluded(&self) -> (Duration, bool) {
        let state = self.lock();
        let current = state
            .paused_since
            .map_or(Duration::ZERO, |since| since.elapsed());
        (state.paused_total + current, state.open_prompts > 0)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ClockState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for PromptPause {
    fn drop(&mut self) {
        let mut state = self.0.lock();
        state.open_prompts = state.open_prompts.saturating_sub(1);
        if state.open_prompts == 0
            && let Some(since) = state.paused_since.take()
        {
            state.paused_total += since.elapsed();
        }
    }
}

/// Run `work` within `budget`, not counting time the `clock` is paused.
pub(crate) async fn within<T>(
    stage: Stage,
    budget: Duration,
    clock: &PromptClock,
    work: impl Future<Output = Result<T, SshError>>,
) -> Result<T, SshError> {
    let start = Instant::now();
    let mut work = pin!(work);
    loop {
        let (excluded, paused) = clock.excluded();
        let used = start.elapsed().saturating_sub(excluded);
        if used >= budget && !paused {
            return Err(SshError::Timeout {
                stage,
                after: budget,
            });
        }
        let wait = if paused {
            budget
        } else {
            budget.saturating_sub(used)
        };
        if let Ok(result) = tokio::time::timeout(wait, &mut work).await {
            return result;
        }
    }
}

#[cfg(test)]
mod tests;
