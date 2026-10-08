// SPDX-License-Identifier: Apache-2.0
//! Which connect stage ran, how long it took and which one failed, for
//! step-by-step Test connection diagnostics (docs/14 §5, M1-07).
use std::future::Future;
use std::time::{Duration, Instant};

use openrows_net::tls::TlsDetails;

use crate::DriverError;

/// A connect stage; drivers record the stages that apply to them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectStage {
    /// Host name resolution (direct TCP only).
    Resolve,
    /// TCP connect, a Unix socket, or a stream through the route (SSH).
    Connect,
    /// TLS negotiation (`SSLRequest`, `MySQL` `SSLRequest` packet) and handshake.
    Tls,
    /// Startup and authentication; for local files, opening the file.
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
    ///
    /// # Errors
    /// The error of `work`, unchanged.
    pub async fn stage<T>(
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
    pub fn restart(&mut self) {
        *self = Self {
            retried_with_tls: true,
            ..Self::default()
        };
    }
}

/// Whether and how the connection is encrypted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TlsStatus {
    /// TLS off (`sslmode=disable`, or `allow` and the server accepted plaintext).
    Disabled,
    /// A Unix-domain socket or a local file: TLS does not apply.
    LocalSocket,
    /// `sslmode=prefer` and the server does not offer TLS: plaintext. Show
    /// this to the user.
    PlaintextFallback,
    /// Encrypted; the details name protocol, cipher and certificate.
    Encrypted(TlsDetails),
}
