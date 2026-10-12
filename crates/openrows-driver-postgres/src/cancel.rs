// SPDX-License-Identifier: Apache-2.0
//! Out-of-band `CancelRequest` over a fresh connection (protocol "Canceling
//! Requests in Progress"), scoped to the statement in flight when requested.
mod statement;
#[cfg(test)]
mod tests;

use crate::connect::{Target, within};
use crate::options::ConnectTimeouts;
use crate::startup::BackendKey;
use bytes::BytesMut;
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError};
use postgres_protocol::message::frontend;
pub(crate) use statement::StatementGuard;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Upper bound for opening the cancel connection and seeing the server close it.
const CANCEL_GRACE: Duration = Duration::from_secs(2);

/// Cancels the statement running on one backend; holds no connection itself.
pub(crate) struct PgCancel {
    target: Target,
    key: BackendKey,
    state: Arc<statement::State>,
}

impl PgCancel {
    pub(crate) fn new(target: Target, key: BackendKey) -> Self {
        Self {
            target,
            key,
            state: Arc::new(statement::State::default()),
        }
    }

    /// Start a statement once every earlier admitted cancel has been processed.
    pub(crate) async fn begin(&self) -> Result<StatementGuard, DriverError> {
        self.state.begin().await
    }
}

impl CancelHandle for PgCancel {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let (target, key, state) = (self.target.clone(), self.key, Arc::clone(&self.state));
        // Bound now, not when polled: a delayed future must not stop later SQL.
        let generation = state.current();
        Box::pin(async move {
            let Some(generation) = generation else {
                return Ok(());
            };
            let Some(_fence) = state.admit(generation).await? else {
                return Ok(());
            };
            within(CANCEL_GRACE, "cancel request", send(&target, key, &state)).await
        })
    }
}

async fn send(
    target: &Target,
    key: BackendKey,
    state: &statement::State,
) -> Result<(), DriverError> {
    let mut request = BytesMut::with_capacity(16);
    frontend::cancel_request(key.process_id, key.secret_key, &mut request);
    let mut stream = target.open(&ConnectTimeouts::default()).await?;
    // From here an error or a dropped future leaves delivery unknown.
    state.sending();
    stream.write_all(&request).await?;
    // The server processes the request before closing this socket; local write
    // completion says nothing about when the backend is signalled (as libpq).
    if stream.read(&mut [0; 1]).await? != 0 {
        return Err(DriverError::Protocol(
            "unexpected data on the cancel connection".to_owned(),
        ));
    }
    state.delivered();
    Ok(())
}
