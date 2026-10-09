// SPDX-License-Identifier: Apache-2.0
//! What a caller holds for one submitted query: its events and cancel handle.
use crate::QueryEvent;
use openrows_types::QueryId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Cancels one query from any thread without waiting for the session actor.
#[derive(Clone, Debug)]
pub struct Canceller(pub(crate) CancellationToken);

impl Canceller {
    /// Request cancellation; idempotent. A queued query never starts; a running
    /// one gets an out-of-band driver cancel and ends with `Cancelled`.
    pub fn cancel(&self) {
        self.0.cancel();
    }
}

/// A submitted query: its id, ordered events and cancel handle.
pub struct QueryHandle {
    pub(crate) id: QueryId,
    pub(crate) events: mpsc::UnboundedReceiver<QueryEvent>,
    pub(crate) canceller: Canceller,
}

impl QueryHandle {
    /// The query's identifier.
    #[must_use]
    pub fn id(&self) -> QueryId {
        self.id
    }

    /// A cloneable handle that cancels this query.
    #[must_use]
    pub fn canceller(&self) -> Canceller {
        self.canceller.clone()
    }

    /// Next event; `None` after the terminal event has been received.
    pub async fn next_event(&mut self) -> Option<QueryEvent> {
        self.events.recv().await
    }
}
