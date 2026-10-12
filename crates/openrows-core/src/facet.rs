// SPDX-License-Identifier: Apache-2.0
//! Catalog and edit calls queued on a session's own connection, for engines
//! whose file admits no second connection (hosted `DuckDB`, ADR-0010).
use crate::shared::Command;
use openrows_driver_api::{BoxFuture, Connection, DriverError};
use openrows_types::SessionId;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// Work the actor runs on its connection; `None` when the session has no facets.
pub(crate) type FacetCall =
    Box<dyn for<'s> FnOnce(Option<&'s mut dyn Connection>) -> BoxFuture<'s, ()> + Send>;

type Reply<T> = oneshot::Receiver<Result<T, DriverError>>;

/// Queues facet calls behind the session's statements; cheap to clone.
#[derive(Clone)]
pub struct FacetPort {
    session: SessionId,
    commands: mpsc::Sender<Command>,
}

impl FacetPort {
    pub(crate) fn new(session: SessionId, commands: mpsc::Sender<Command>) -> Self {
        Self { session, commands }
    }

    /// Run `call` on the connection once the statements queued before it end.
    ///
    /// # Errors
    /// `call`'s error; `Unsupported` when the session has no facets; `Host`
    /// failure text as `Unsupported` when the session closed or failed first.
    pub async fn call<T, F>(&self, call: F) -> Result<T, DriverError>
    where
        T: Send + 'static,
        F: for<'s> FnOnce(&'s mut dyn Connection) -> BoxFuture<'s, Result<T, DriverError>>
            + Send
            + 'static,
    {
        let received = self.enqueue(call, false).await?;
        received.await.map_err(|_| self.closed())?
    }

    /// Like [`FacetPort::call`], but wait at most `limit`: `Ok(None)` while
    /// the session is still busy. A queued call whose caller stopped waiting
    /// is skipped, so abandoned calls never pile up behind statements.
    ///
    /// # Errors
    /// As [`FacetPort::call`].
    pub async fn call_within<T, F>(
        &self,
        limit: Duration,
        call: F,
    ) -> Result<Option<T>, DriverError>
    where
        T: Send + 'static,
        F: for<'s> FnOnce(&'s mut dyn Connection) -> BoxFuture<'s, Result<T, DriverError>>
            + Send
            + 'static,
    {
        let waited = tokio::time::timeout(limit, async {
            let received = self.enqueue(call, true).await?;
            received.await.map_err(|_| self.closed())?
        })
        .await;
        waited.map_or(Ok(None), |result| result.map(Some))
    }

    async fn enqueue<T, F>(&self, call: F, skip_abandoned: bool) -> Result<Reply<T>, DriverError>
    where
        T: Send + 'static,
        F: for<'s> FnOnce(&'s mut dyn Connection) -> BoxFuture<'s, Result<T, DriverError>>
            + Send
            + 'static,
    {
        let (reply, received) = oneshot::channel();
        let facet: FacetCall = Box::new(move |connection: Option<&mut dyn Connection>| {
            if skip_abandoned && reply.is_closed() {
                return Box::pin(std::future::ready(()));
            }
            Box::pin(async move {
                let result = match connection {
                    Some(connection) => call(connection).await,
                    None => Err(DriverError::Unsupported(
                        "this session has no catalog or editor".into(),
                    )),
                };
                // The caller may have stopped waiting; nothing else to notify.
                let _ = reply.send(result);
            })
        });
        self.commands
            .send(Command::Facet(facet))
            .await
            .map_err(|_| self.closed())?;
        Ok(received)
    }

    fn closed(&self) -> DriverError {
        DriverError::Unsupported(format!(
            "session {} closed before the catalog or edit call ran; reconnect",
            self.session
        ))
    }
}
