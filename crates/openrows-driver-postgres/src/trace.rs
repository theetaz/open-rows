// SPDX-License-Identifier: Apache-2.0
//! Which connect stage ran, how long it took and which one failed, for
//! step-by-step Test connection diagnostics (docs/14 §5, M1-07).
use std::future::Future;
use std::time::{Duration, Instant};

use openrows_driver_api::DriverError;

use crate::info::TlsStatus;

/// A connect stage of the `PostgreSQL` driver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectStage {
    /// Host name resolution (direct TCP only).
    Resolve,
    /// TCP connect, a Unix socket, or a stream through the route (SSH).
    Connect,
    /// `SSLRequest` and the TLS handshake.
    Tls,
    /// Startup and authentication up to the first `ReadyForQuery`.
    Authenticate,
    /// The server version and identity query.
    ServerInfo,
    /// The profile's startup commands.
    StartupCommands,
}

/// What happened during one connect.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConnectTrace {
    /// Completed stages in order with their duration.
    pub completed: Vec<(ConnectStage, Duration)>,
    /// The stage that failed, if the connect failed.
    pub failed: Option<ConnectStage>,
    /// Transport encryption, known once negotiation finished.
    pub tls: Option<TlsStatus>,
    /// `sslmode=allow` retried with TLS after the plaintext session was rejected.
    pub retried_with_tls: bool,
}

impl ConnectTrace {
    /// Run `work` as `stage`, recording its duration or its failure.
    pub(crate) async fn stage<T>(
        &mut self,
        stage: ConnectStage,
        work: impl Future<Output = Result<T, DriverError>>,
    ) -> Result<T, DriverError> {
        let started = Instant::now();
        let result = work.await;
        match &result {
            Ok(_) => self.completed.push((stage, started.elapsed())),
            Err(_) => self.failed = Some(stage),
        }
        result
    }

    /// Start over for a retry, remembering that one happened.
    pub(crate) fn restart(&mut self) {
        *self = Self {
            retried_with_tls: true,
            ..Self::default()
        };
    }
}
