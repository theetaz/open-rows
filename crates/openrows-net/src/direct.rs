// SPDX-License-Identifier: Apache-2.0
//! Direct TCP streams for protocols without a database-specific negotiation.
use crate::{BoxFuture, BoxStream, Dialer, configure_tcp};
use std::io;
use tokio::net::TcpStream;

/// A direct TCP destination, also usable by HTTP protocol drivers.
#[derive(Clone, Debug)]
pub struct TcpDialer {
    host: String,
    port: u16,
}

impl TcpDialer {
    /// Resolve and connect to this host and port when dialled.
    #[must_use]
    pub fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }
}

impl Dialer for TcpDialer {
    fn dial(&self) -> BoxFuture<'_, io::Result<BoxStream>> {
        Box::pin(async move {
            let stream = TcpStream::connect((self.host.as_str(), self.port)).await?;
            configure_tcp(&stream)?;
            Ok(Box::new(stream) as BoxStream)
        })
    }

    fn describe(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
