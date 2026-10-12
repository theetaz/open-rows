// SPDX-License-Identifier: Apache-2.0
//! Owned diagnostic shutdown has a deadline; dropping a timed-out close drops its transport.
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError, ExecSummary, ResultSink, Session};
use std::{sync::Arc, time::Duration};

/// A disposable `PostgreSQL` session with bounded graceful shutdown.
pub(super) struct DiagnosticSession(pub(super) Box<dyn Session>);
impl Session for DiagnosticSession {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async move { self.0.execute(sql, sink).await.map_err(sanitized) })
    }
    fn execute_bound<'a>(
        &'a mut self,
        sql: &'a str,
        params: &'a [Option<String>],
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async move {
            self.0
                .execute_bound(sql, params, sink)
                .await
                .map_err(sanitized)
        })
    }
    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        self.0.cancel_handle()
    }
    fn host_failure(&self) -> Option<BoxFuture<'static, DriverError>> {
        self.0.host_failure().map(|failure| {
            Box::pin(async move { sanitized(failure.await) }) as BoxFuture<'static, DriverError>
        })
    }
    fn ping(&mut self) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(async move { self.0.ping().await.map_err(sanitized) })
    }
    fn in_transaction(&self) -> bool {
        self.0.in_transaction()
    }
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async move {
            tokio::time::timeout(Duration::from_secs(2), self.0.close())
                .await
                .unwrap_or_else(|_| {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Diagnostic transport closed after shutdown deadline",
                    )
                    .into())
                })
                .map_err(sanitized)
        })
    }
}

// Session failures are broadcast independently of the diagnostic command reply.
fn sanitized(error: DriverError) -> DriverError {
    let message = "Diagnostic failed; raw server/transport details withheld";
    match error {
        DriverError::Host(failure) => DriverError::Host(failure),
        DriverError::Network(error) => std::io::Error::new(error.kind(), message).into(),
        DriverError::Tls(_) => DriverError::Tls(message.into()),
        DriverError::Auth(_) => DriverError::Auth(message.into()),
        DriverError::Protocol(_) => DriverError::Protocol(message.into()),
        DriverError::Decode { column, .. } => DriverError::Decode {
            column,
            reason: message.into(),
        },
        DriverError::Unsupported(_) => DriverError::Unsupported(message.into()),
        DriverError::Arrow(_) => arrow_schema::ArrowError::ParseError(message.into()).into(),
        DriverError::Server { code, message: raw } => {
            let message = if code == "57014" && raw.contains("statement timeout") {
                "Diagnostic statement timeout"
            } else {
                message
            };
            let code = if code.len() == 5 && code.bytes().all(|c| c.is_ascii_alphanumeric()) {
                code
            } else {
                "XX000".into()
            };
            DriverError::Server {
                code,
                message: message.into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    #[test]
    fn diagnostic_failures_are_redacted_before_shared_session_ipc() -> Result<(), serde_json::Error>
    {
        for error in [
            DriverError::Server {
                code: "42P01".into(),
                message: "test-only".into(),
            },
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "test-only").into(),
            DriverError::Protocol("test-only".into()),
            DriverError::Decode {
                column: 1,
                reason: "test-only".into(),
            },
            arrow_schema::ArrowError::ParseError("test-only".into()).into(),
        ] {
            let event = crate::connections::sessions::SessionStateChanged::from(
                openrows_core::SessionEvent {
                    session: openrows_types::SessionId::new(),
                    state: openrows_core::SessionState::Failed,
                    error: Some(openrows_core::CoreError::from(&sanitized(error))),
                },
            );
            assert!(!serde_json::to_string(&event)?.contains("test-only"));
        }
        let timeout = sanitized(DriverError::Server {
            code: "57014".into(),
            message: "statement timeout: test-only".into(),
        });
        assert_eq!(
            openrows_core::CoreError::from(&timeout).kind,
            openrows_core::ErrorKind::Timeout
        );
        Ok(())
    }
    struct HungClose(Arc<AtomicBool>);
    impl Drop for HungClose {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    impl Session for HungClose {
        fn execute<'a>(
            &'a mut self,
            _: &'a str,
            _: &'a mut dyn ResultSink,
        ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
            Box::pin(async { Ok(ExecSummary::default()) })
        }
        fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
            Box::pin(async move {
                let _owned = self;
                std::future::pending().await
            })
        }
    }
    #[tokio::test(start_paused = true)]
    async fn stalled_diagnostic_shutdown_drops_owned_transport_with_typed_timeout() {
        let dropped = Arc::new(AtomicBool::new(false));
        let closing =
            tokio::spawn(Box::new(DiagnosticSession(Box::new(HungClose(dropped.clone())))).close());
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(
            matches!(closing.await.expect("fixture task"), Err(DriverError::Network(error)) if error.kind() == std::io::ErrorKind::TimedOut)
        );
        assert!(dropped.load(Ordering::SeqCst));
    }
}
