// SPDX-License-Identifier: Apache-2.0
//! Database streams through a proxy (no SSH): one proxy handshake per dial.

use std::{fmt, io, sync::Arc};

use super::ProxySpec;
use crate::{BoxFuture, BoxStream, Dialer, Timeouts};

/// Opens streams to `host:port` through a SOCKS5 or HTTP `CONNECT` proxy.
#[derive(Clone)]
pub struct ProxyDialer {
    proxy: Arc<ProxySpec>,
    host: String,
    port: u16,
    timeouts: Timeouts,
}

impl ProxyDialer {
    /// Reach `host:port` (resolved by the proxy) through `proxy`.
    #[must_use]
    pub fn new(proxy: Arc<ProxySpec>, host: &str, port: u16) -> Self {
        Self {
            proxy,
            host: host.to_owned(),
            port,
            timeouts: Timeouts::default(),
        }
    }
}

impl Dialer for ProxyDialer {
    fn dial(&self) -> BoxFuture<'_, io::Result<BoxStream>> {
        Box::pin(async move {
            let stream = self
                .proxy
                .connect(&self.host, self.port, &self.timeouts)
                .await
                .map_err(io::Error::other)?;
            let stream: BoxStream = Box::new(stream);
            Ok(stream)
        })
    }

    fn describe(&self) -> String {
        format!("{} → {}:{}", self.proxy.describe(), self.host, self.port)
    }
}

impl fmt::Debug for ProxyDialer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.describe())
    }
}
