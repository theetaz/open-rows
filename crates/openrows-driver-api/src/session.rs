// SPDX-License-Identifier: Apache-2.0
use crate::{DiscardSink, DriverError, ExecSummary, ResultSink};
use std::{future::Future, pin::Pin, sync::Arc};

/// Boxed `Send` future, keeping [`Session`] object-safe without extra crates.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Out-of-band cancellation for whatever statement is running on one session.
///
/// Obtained before execution and held outside the session actor, so a cancel
/// request never waits behind the running statement (ADR-0007, docs/04 §6).
pub trait CancelHandle: Send + Sync {
    /// Ask the server to abort the current statement; the running `execute`
    /// then finishes with an error and the session stays usable.
    ///
    /// # Errors
    /// Transport failures while delivering the request.
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>>;
}

/// One live connection, exclusively owned by its session actor (ADR-0007).
///
/// `Send` but not `Sync`: only the owning actor task ever touches it.
pub trait Session: Send {
    /// Execute one parameterless statement, streaming any rows into `sink`.
    ///
    /// # Errors
    /// `DriverError::Server` leaves the session usable; transport and protocol
    /// errors mean the actor must discard it.
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>>;

    /// Execute one statement with positional text parameters (`$1`, `$2`, …;
    /// `None` is SQL NULL). The server parses each value with the input
    /// function of the type it infers for the placeholder.
    ///
    /// The default supports only an empty parameter list.
    ///
    /// # Errors
    /// As [`Session::execute`]; `Unsupported` when the driver cannot bind.
    fn execute_bound<'a>(
        &'a mut self,
        sql: &'a str,
        params: &'a [Option<String>],
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        if params.is_empty() {
            return self.execute(sql, sink);
        }
        Box::pin(async { Err(DriverError::Unsupported("bind parameters".to_owned())) })
    }

    /// Out-of-band cancel handle; `None` when the driver cannot cancel server-side.
    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        None
    }

    /// Cheapest round trip that proves the connection still answers: the
    /// keepalive probe of an idle session (docs/14 §2). The default runs
    /// `SELECT 1`; drivers may use a lighter protocol message.
    ///
    /// # Errors
    /// Transport errors when the connection is gone; a `Server` error (e.g.
    /// an aborted transaction) still proves it is alive.
    fn ping(&mut self) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(async move { self.execute("SELECT 1", &mut DiscardSink).await.map(drop) })
    }

    /// True while the server holds a transaction open on this connection,
    /// as of the last completed statement. Losing such a connection loses
    /// the transaction, so it must never be replaced silently.
    fn in_transaction(&self) -> bool {
        false
    }

    /// Gracefully terminate the connection.
    ///
    /// # Errors
    /// Returns transport failures while sending the termination message.
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SinkFlow;
    use arrow_array::RecordBatch;
    use arrow_schema::{Schema, SchemaRef};
    use std::{
        sync::Arc,
        task::{Context, Poll, Waker},
    };

    struct Echo;

    impl Session for Echo {
        fn execute<'a>(
            &'a mut self,
            _sql: &'a str,
            sink: &'a mut dyn ResultSink,
        ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
            Box::pin(async move {
                let schema = Arc::new(Schema::empty());
                sink.begin_result(schema.clone());
                let flow = sink.push_batch(RecordBatch::new_empty(schema));
                Ok(ExecSummary {
                    batches: u64::from(flow == SinkFlow::Continue),
                    ..ExecSummary::default()
                })
            })
        }

        fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
            Box::pin(async { Ok(()) })
        }
    }

    #[derive(Default)]
    struct Count(usize);

    impl ResultSink for Count {
        fn begin_result(&mut self, _schema: SchemaRef) {}
        fn push_batch(&mut self, _batch: RecordBatch) -> SinkFlow {
            self.0 += 1;
            SinkFlow::Continue
        }
    }

    fn assert_send<T: Send>(_: &T) {}

    fn ready<T>(mut future: BoxFuture<'_, T>) -> T {
        let poll = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
        let Poll::Ready(value) = poll else {
            unreachable!("test session never suspends")
        };
        value
    }

    #[test]
    fn boxed_session_is_object_safe_and_send() -> Result<(), DriverError> {
        let mut session: Box<dyn Session> = Box::new(Echo);
        assert_send(&session);
        let mut sink = Count::default();
        let summary = ready(session.execute("select", &mut sink))?;
        assert_eq!((summary.batches, sink.0), (1, 1));
        assert!(session.cancel_handle().is_none());
        let bound = ready(session.execute_bound("select", &[], &mut sink))?;
        assert_eq!(bound.batches, 1);
        let refused = ready(session.execute_bound("select $1", &[None], &mut sink));
        assert!(matches!(refused, Err(DriverError::Unsupported(_))));
        ready(session.ping())?;
        assert!(!session.in_transaction());
        ready(session.close())
    }
}
