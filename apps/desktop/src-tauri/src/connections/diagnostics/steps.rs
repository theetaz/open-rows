// SPDX-License-Identifier: Apache-2.0
//! Test connection steps: SSH hops from the tunnel attempt, then the driver's
//! connect trace (DNS, TCP or tunnel stream, TLS, authentication, server
//! version). A failed step carries the typed error and a hint on what to
//! change ([`super::hints`]).
use std::time::Duration;

use openrows_driver_api::DriverError;
use openrows_driver_api::{ConnectStage, ConnectTrace, TlsStatus};
use openrows_net::{SshError, Stage};
use openrows_store::profile::{Driver as Engine, is_local_host};
use serde::Serialize;
use specta::Type;

use super::super::error::IpcError;
use super::super::target::ServerDto;
use super::super::tunnels::{TunnelAttempt, ssh_error};
use super::hints::hint;
use super::probe::expiry_note;
use labels::describe;

/// What a step checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TestStepKind {
    /// Launch and health-check a cloud helper.
    Helper,
    /// Resolve and reach an SSH server (or the next jump host).
    SshConnect,
    /// SSH handshake, host key and authentication.
    SshAuth,
    /// Open the tunnel stream to the database.
    Tunnel,
    /// Resolve the database host name (direct connections).
    Dns,
    /// TCP connect (or Unix socket) to the database.
    Connect,
    /// TLS negotiation and handshake.
    Tls,
    /// Database authentication.
    Authenticate,
    /// Server product and version.
    Version,
    /// Round trip of a trivial query.
    Latency,
}

/// How a step went.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum StepStatus {
    Ok,
    /// Succeeded, but insecure or degraded (e.g. TLS without verification).
    Warning,
    Failed,
}

/// One line of the Test connection report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestStepDto {
    pub(crate) kind: TestStepKind,
    pub(crate) label: String,
    pub(crate) status: StepStatus,
    pub(crate) detail: Option<String>,
    /// What to change when the step failed.
    pub(crate) hint: Option<String>,
    pub(crate) elapsed_ms: Option<u32>,
}

/// Steps collected so far, for a profile of `engine`.
#[derive(Default)]
pub(crate) struct Steps {
    pub(crate) list: Vec<TestStepDto>,
    pub(crate) engine: Engine,
    /// `SOCKS5 proxy host:port` in front of the first hop, if any.
    pub(crate) proxy: Option<String>,
}

impl Steps {
    /// Command line, version and last scrubbed stderr of a cloud helper.
    pub(crate) fn helper(
        &mut self,
        hop: &openrows_net::helper::HelperHop,
        result: &Result<std::sync::Arc<openrows_net::helper::Helper>, IpcError>,
        elapsed: Duration,
    ) {
        let step = self.timed(
            TestStepKind::Helper,
            format!("Start {}", hop.tool()),
            elapsed,
        );
        step.hint = Some(hop.hint().into());
        match result {
            Ok(helper) => {
                step.detail = Some(
                    helper
                        .diagnostics()
                        .unwrap_or_else(|error| error.to_string()),
                );
            }
            Err(error) => {
                step.status = StepStatus::Failed;
                step.detail = Some(error.message.clone());
            }
        }
    }
    fn push(&mut self, kind: TestStepKind, label: String, status: StepStatus) -> &mut TestStepDto {
        // The label and detail name hosts and users; the log keeps only the step.
        tracing::debug!(step = ?kind, status = ?status, "connection test step");
        self.list.push(TestStepDto {
            kind,
            label,
            status,
            detail: None,
            hint: None,
            elapsed_ms: None,
        });
        let last = self.list.len() - 1;
        &mut self.list[last]
    }

    fn timed(&mut self, kind: TestStepKind, label: String, elapsed: Duration) -> &mut TestStepDto {
        let step = self.push(kind, label, StepStatus::Ok);
        step.elapsed_ms = Some(millis(elapsed));
        step
    }

    fn failed(&mut self, kind: TestStepKind, label: String, error: &IpcError) {
        let advice = hint(kind, error, self.engine).map(str::to_owned);
        let step = self.push(kind, label, StepStatus::Failed);
        step.detail = Some(error.message.clone());
        step.hint = advice;
    }

    /// One connect and one authentication step per hop that came up; the
    /// failing hop's step carries the typed error.
    pub(crate) fn ssh(&mut self, attempt: &TunnelAttempt) -> Option<IpcError> {
        for (index, timing) in attempt.completed.iter().enumerate() {
            let Some(hop) = attempt.hops.get(index) else {
                break;
            };
            let label = self.reach(index, &hop.host, hop.port);
            self.timed(TestStepKind::SshConnect, label, timing.connect);
            let label = format!("SSH login as {}@{}", hop.user, hop.host);
            self.timed(TestStepKind::SshAuth, label, timing.handshake);
        }
        let error = attempt.result.as_ref().err()?;
        let index = attempt.completed.len();
        let typed = ssh_error(error);
        let (host, port, user) = attempt.hops.get(index).map_or_else(
            || (String::new(), 0, String::new()),
            |hop| (hop.host.clone(), hop.port, hop.user.clone()),
        );
        if fails_to_reach(error) {
            let label = self.reach(index, &host, port);
            self.failed(TestStepKind::SshConnect, label, &typed);
        } else {
            self.failed(
                TestStepKind::SshAuth,
                format!("SSH login as {user}@{host}"),
                &typed,
            );
        }
        Some(typed)
    }

    /// Steps from the driver's trace; `failure` is the connect error, if any.
    pub(crate) fn database(&mut self, run: &DatabaseRun<'_>, failure: Option<&DriverError>) {
        for (stage, elapsed) in &run.trace.completed {
            match stage {
                ConnectStage::Resolve => {
                    let label = format!("Resolve {}", run.host);
                    self.timed(TestStepKind::Dns, label, *elapsed);
                }
                ConnectStage::Connect => self.connected(run, *elapsed),
                ConnectStage::Tls => self.tls(run, *elapsed),
                ConnectStage::Authenticate => {
                    let label = format!("Authenticate as {}", run.user);
                    self.timed(TestStepKind::Authenticate, label, *elapsed);
                }
                ConnectStage::ServerInfo => {
                    let label = "Server version".to_owned();
                    let step = self.timed(TestStepKind::Version, label, *elapsed);
                    step.detail = run.server.map(ServerDto::version_label);
                }
                ConnectStage::StartupCommands => {}
            }
        }
        if let (Some(stage), Some(error)) = (run.trace.failed, failure) {
            let (kind, label) = run.failed_step(stage);
            self.failed(kind, label, &IpcError::from(error));
        }
    }

    /// The connect or tunnel step, then the TLS step when TLS was not
    /// negotiated (files have none).
    fn connected(&mut self, run: &DatabaseRun<'_>, elapsed: Duration) {
        let (kind, label) = run.connect_step();
        self.timed(kind, label, elapsed);
        if run.file {
            return;
        }
        let negotiated = run
            .trace
            .completed
            .iter()
            .any(|(stage, _)| *stage == ConnectStage::Tls);
        if !negotiated {
            self.tls_skipped(run);
        }
    }

    fn tls(&mut self, run: &DatabaseRun<'_>, elapsed: Duration) {
        let step = self.timed(TestStepKind::Tls, "TLS handshake".to_owned(), elapsed);
        match &run.trace.tls {
            Some(TlsStatus::Encrypted(details)) => {
                let expiry = expiry_note(details);
                let unverified = details.verification.is_skipped() && !is_local_host(run.host);
                if unverified || expiry.is_some() {
                    step.status = StepStatus::Warning;
                }
                let mut detail = describe(details);
                if let Some(expiry) = expiry {
                    detail = format!("{expiry} · {detail}");
                }
                step.detail = Some(detail);
            }
            Some(TlsStatus::PlaintextFallback) => {
                step.status = StepStatus::Warning;
                step.detail =
                    Some("The server does not offer TLS; connected without encryption".to_owned());
            }
            Some(TlsStatus::Disabled | TlsStatus::LocalSocket) | None => {}
        }
    }

    /// "Connect to SSH server …" (through the proxy for the first hop) or
    /// "Reach jump target …".
    fn reach(&self, index: usize, host: &str, port: u16) -> String {
        match (index, &self.proxy) {
            (0, Some(proxy)) => format!("Connect to SSH server {host}:{port} via {proxy}"),
            (0, None) => format!("Connect to SSH server {host}:{port}"),
            _ => format!("Reach jump target {host}:{port}"),
        }
    }

    fn tls_skipped(&mut self, run: &DatabaseRun<'_>) {
        let (status, detail) = match run.trace.tls {
            Some(TlsStatus::LocalSocket) => (StepStatus::Ok, "Not used for Unix sockets"),
            _ if is_local_host(run.host) || run.tunnelled => (
                StepStatus::Ok,
                "Off; the connection is local or inside the SSH tunnel",
            ),
            _ => (
                StepStatus::Warning,
                "Off; traffic to the server is not encrypted",
            ),
        };
        self.push(TestStepKind::Tls, "TLS".to_owned(), status)
            .detail = Some(detail.to_owned());
    }
}

/// What the database part of a test needs to describe itself.
pub(crate) struct DatabaseRun<'a> {
    pub(crate) trace: &'a ConnectTrace,
    pub(crate) host: &'a str,
    pub(crate) port: u16,
    pub(crate) user: &'a str,
    pub(crate) tunnelled: bool,
    /// The Unix socket on the SSH server the tunnel reaches, if not a port.
    pub(crate) socket: Option<&'a str>,
    /// `SOCKS5 proxy host:port` carrying a direct connection.
    pub(crate) proxy: Option<&'a str>,
    /// A local database file: `host` is its path and there is no TLS.
    pub(crate) file: bool,
    pub(crate) server: Option<&'a ServerDto>,
}

impl DatabaseRun<'_> {
    fn connect_step(&self) -> (TestStepKind, String) {
        if self.file {
            return (TestStepKind::Connect, format!("Open {}", self.host));
        }
        let target = self
            .socket
            .map_or_else(|| format!("{}:{}", self.host, self.port), str::to_owned);
        match (self.tunnelled, self.proxy) {
            (true, _) => (TestStepKind::Tunnel, format!("Open tunnel to {target}")),
            (false, Some(proxy)) => (
                TestStepKind::Connect,
                format!("Connect to {target} via {proxy}"),
            ),
            (false, None) => (TestStepKind::Connect, format!("Connect to {target}")),
        }
    }

    fn failed_step(&self, stage: ConnectStage) -> (TestStepKind, String) {
        match stage {
            ConnectStage::Resolve => (TestStepKind::Dns, format!("Resolve {}", self.host)),
            ConnectStage::Connect => self.connect_step(),
            ConnectStage::Tls => (TestStepKind::Tls, "TLS handshake".to_owned()),
            ConnectStage::Authenticate | ConnectStage::StartupCommands => (
                TestStepKind::Authenticate,
                format!("Authenticate as {}", self.user),
            ),
            ConnectStage::ServerInfo => (TestStepKind::Version, "Server version".to_owned()),
        }
    }
}

/// Failures before the SSH handshake: DNS, TCP or the channel to the next hop.
fn fails_to_reach(error: &SshError) -> bool {
    matches!(
        error,
        SshError::Resolve { .. }
            | SshError::Connect { .. }
            | SshError::ChannelOpen { .. }
            | SshError::EmptyRoute
            | SshError::Proxy(_)
            | SshError::Timeout {
                stage: Stage::Resolve | Stage::TcpConnect | Stage::ChannelOpen | Stage::Proxy,
                ..
            }
    )
}

fn millis(duration: Duration) -> u32 {
    u32::try_from(duration.as_millis()).unwrap_or(u32::MAX)
}

mod labels;
#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod tests;
