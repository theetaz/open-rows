// SPDX-License-Identifier: Apache-2.0
//! Out-of-band `CancelRequest` over a fresh connection (protocol §53.2.7).
use crate::connect::Target;
use crate::options::ConnectTimeouts;
use crate::startup::BackendKey;
use bytes::BytesMut;
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError};
use postgres_protocol::message::frontend;
use tokio::io::AsyncWriteExt;

/// Cancels the statement running on one backend; holds no connection itself.
pub(crate) struct PgCancel {
    target: Target,
    key: BackendKey,
}

impl PgCancel {
    pub(crate) fn new(target: Target, key: BackendKey) -> Self {
        Self { target, key }
    }
}

impl CancelHandle for PgCancel {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let (target, key) = (self.target.clone(), self.key);
        Box::pin(async move {
            let mut request = BytesMut::with_capacity(16);
            frontend::cancel_request(key.process_id, key.secret_key, &mut request);
            let mut stream = target.open(&ConnectTimeouts::default()).await?;
            stream.write_all(&request).await?;
            stream.shutdown().await?;
            Ok(())
        })
    }
}
