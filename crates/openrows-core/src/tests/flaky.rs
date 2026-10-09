// SPDX-License-Identifier: Apache-2.0
//! A scripted server whose connections can be killed, black-holed or
//! refused, for keepalive and reconnect tests.
use crate::{Reconnect, Resilience, RetryPolicy};
use openrows_driver_api::{BoxFuture, DriverError, ExecSummary, ResultSink, Session};
use openrows_sql::Dialect;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// Server state shared by every connection it accepted.
#[derive(Default)]
pub(super) struct FlakyServer {
    /// Connections opened before the last kill are dead.
    generation: AtomicUsize,
    refuse: AtomicBool,
    silent: AtomicBool,
    connects: AtomicUsize,
    log: Mutex<Vec<String>>,
}

impl FlakyServer {
    /// Kill every open connection, as `pg_terminate_backend` or a restart does.
    pub(super) fn kill_connections(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Refuse new connections (server down) or accept them again.
    pub(super) fn set_refusing(&self, on: bool) {
        self.refuse.store(on, Ordering::SeqCst);
    }

    /// Stop answering on open connections (half-open link) or answer again.
    pub(super) fn set_silent(&self, on: bool) {
        self.silent.store(on, Ordering::SeqCst);
    }

    pub(super) fn connects(&self) -> usize {
        self.connects.load(Ordering::SeqCst)
    }

    /// Statements the server applied, in order.
    pub(super) fn applied(&self) -> Vec<String> {
        self.lock().clone()
    }

    /// Opens a connection to this server.
    pub(super) fn reconnect(self: &Arc<Self>) -> Reconnect {
        let server = Arc::clone(self);
        Arc::new(move || Box::pin(std::future::ready(Arc::clone(&server).accept())))
    }

    fn accept(self: Arc<Self>) -> Result<Box<dyn Session>, DriverError> {
        if self.refuse.load(Ordering::SeqCst) {
            return Err(io::Error::from(io::ErrorKind::ConnectionRefused).into());
        }
        self.connects.fetch_add(1, Ordering::SeqCst);
        let generation = self.generation.load(Ordering::SeqCst);
        Ok(Box::new(FlakyConnection {
            server: self,
            generation,
            in_transaction: false,
        }))
    }

    /// Fast test timings, replaying `PostgreSQL` search paths.
    pub(super) fn resilience(self: &Arc<Self>) -> Resilience {
        Resilience {
            reconnect: self.reconnect(),
            dialect: Some(Dialect::Postgres),
            policy: RetryPolicy {
                probe_timeout: Duration::from_millis(100),
                stale_after: Duration::from_secs(5),
                first_delay: Duration::from_millis(10),
                max_delay: Duration::from_millis(40),
                attempts: 3,
            },
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
        self.log.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// One connection; understands `begin`, `commit`, `kill-after <sql>` (applies
/// the statement, then the connection dies before the reply) and any other
/// text, which it just records.
struct FlakyConnection {
    server: Arc<FlakyServer>,
    generation: usize,
    in_transaction: bool,
}

impl FlakyConnection {
    async fn answer(&self) -> Result<(), DriverError> {
        if self.server.silent.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if self.generation == self.server.generation.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(io::Error::from(io::ErrorKind::UnexpectedEof).into())
        }
    }

    async fn run(&mut self, sql: &str) -> Result<ExecSummary, DriverError> {
        self.answer().await?;
        let applied = sql.strip_prefix("kill-after ").unwrap_or(sql);
        self.server.lock().push(applied.to_owned());
        if applied != sql {
            // The reply (and the new transaction status) never arrives.
            self.server.kill_connections();
            return Err(io::Error::from(io::ErrorKind::ConnectionReset).into());
        }
        match applied {
            "begin" => self.in_transaction = true,
            "commit" => self.in_transaction = false,
            _ => {}
        }
        Ok(ExecSummary::default())
    }
}

impl Session for FlakyConnection {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        _sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run(sql))
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(self.answer())
    }

    fn in_transaction(&self) -> bool {
        self.in_transaction
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async { Ok(()) })
    }
}
