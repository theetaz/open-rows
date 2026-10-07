// SPDX-License-Identifier: Apache-2.0
use crate::{DriverError, ExecSummary, ResultSink};
use std::{future::Future, pin::Pin};

/// Boxed `Send` future, keeping [`Session`] object-safe without extra crates.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

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
        ready(session.close())
    }
}
