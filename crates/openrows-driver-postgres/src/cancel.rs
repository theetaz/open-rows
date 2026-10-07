// SPDX-License-Identifier: Apache-2.0
//! Out-of-band `CancelRequest` over a fresh TCP connection (protocol §53.2.7).
use crate::startup::BackendKey;
use bytes::BytesMut;
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError};
use postgres_protocol::message::frontend;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

/// Cancels the statement running on one backend; holds no connection itself.
pub(crate) struct PgCancel {
    host: String,
    port: u16,
    key: BackendKey,
}

impl PgCancel {
    pub(crate) fn new(host: String, port: u16, key: BackendKey) -> Self {
        Self { host, port, key }
    }
}

impl CancelHandle for PgCancel {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        let (host, port, key) = (self.host.clone(), self.port, self.key);
        Box::pin(async move {
            let mut request = BytesMut::with_capacity(16);
            frontend::cancel_request(key.process_id, key.secret_key, &mut request);
            let mut stream = TcpStream::connect((host.as_str(), port)).await?;
            stream.write_all(&request).await?;
            stream.shutdown().await?;
            Ok(())
        })
    }
}
