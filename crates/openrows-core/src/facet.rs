// SPDX-License-Identifier: Apache-2.0
//! Catalog and edit calls queued on a session's own connection, for engines
//! whose file admits no second connection (hosted `DuckDB`, ADR-0010).
use crate::shared::Command;
use openrows_driver_api::{BoxFuture, Connection, DriverError};
use openrows_types::SessionId;
use tokio::sync::{mpsc, oneshot};

/// Work the actor runs on its connection; `None` when the session has no facets.
pub(crate) type FacetCall =
    Box<dyn for<'s> FnOnce(Option<&'s mut dyn Connection>) -> BoxFuture<'s, ()> + Send>;

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
        let (reply, received) = oneshot::channel();
        let facet: FacetCall = Box::new(move |connection: Option<&mut dyn Connection>| {
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
        received.await.map_err(|_| self.closed())?
    }

    fn closed(&self) -> DriverError {
        DriverError::Unsupported(format!(
            "session {} closed before the catalog or edit call ran; reconnect",
            self.session
        ))
    }
}
