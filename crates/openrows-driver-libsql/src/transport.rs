// SPDX-License-Identifier: Apache-2.0
//! TLS, proxies and SSH stay in openrows-net; HTTP never follows redirects.
use openrows_driver_api::{
    ConnectRequest, ConnectStage, ConnectTrace, DriverError, Endpoint, SslMode, TlsStatus,
};
use openrows_net::{BoxStream, Dialer, TcpDialer, tls::TlsClient};
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub(crate) struct Transport {
    pub(crate) url: Url,
    route: Arc<dyn Dialer>,
    tls: Option<TlsClient>,
    timeouts: openrows_driver_api::ConnectTimeouts,
}

impl Transport {
    pub(crate) fn new(request: &ConnectRequest) -> Result<Self, DriverError> {
        let Endpoint::Server { host, port } = &request.endpoint else {
            return Err(DriverError::Unsupported(
                "remote libSQL requires a server".into(),
            ));
        };
        if host.is_empty()
            || *port == 0
            || host.contains(['/', '@', '?', '#', '%', '\\'])
            || host.chars().any(char::is_whitespace)
            || (host.contains(':') && host.parse::<std::net::Ipv6Addr>().is_err())
        {
            return Err(DriverError::Unsupported(
                "invalid libSQL host; use the auth token field".into(),
            ));
        }
        let scheme = if request.ssl_mode == SslMode::Disable {
            "http"
        } else {
            "https"
        };
        let host = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.clone()
        };
        let url = Url::parse(&format!("{scheme}://{host}:{port}/"))
            .map_err(|_| DriverError::Unsupported("invalid libSQL host".into()))?;
        let host = url
            .host_str()
            .ok_or_else(crate::error::protocol)?
            .trim_matches(['[', ']']);
        let route = request
            .via
            .clone()
            .unwrap_or_else(|| Arc::new(TcpDialer::new(host.into(), *port)));
        let tls = (scheme == "https")
            .then(|| TlsClient::new(&request.tls.params(request.ssl_mode)))
            .transpose()
            .map_err(|_| DriverError::Tls("libSQL TLS configuration failed".into()))?;
        Ok(Self {
            url,
            route,
            tls,
            timeouts: request.timeouts,
        })
    }

    pub(crate) async fn open(&self, trace: &mut ConnectTrace) -> Result<BoxStream, DriverError> {
        let dial = async {
            tokio::time::timeout(self.timeouts.tcp, self.route.dial())
                .await
                .map_err(|_| crate::error::lost("libSQL connect timed out"))?
                .map_err(|error| {
                    DriverError::Network(std::io::Error::new(error.kind(), "libSQL route failed"))
                })
        };
        let stream = trace.stage(ConnectStage::Connect, dial).await?;
        let Some(tls) = &self.tls else {
            trace.tls = Some(TlsStatus::Disabled);
            return Ok(stream);
        };
        let host = self
            .url
            .host_str()
            .ok_or_else(crate::error::protocol)?
            .trim_matches(['[', ']']);
        let handshake = async {
            tokio::time::timeout(self.timeouts.tls, tls.handshake(stream, host))
                .await
                .map_err(|_| DriverError::Tls("libSQL TLS handshake timed out".into()))?
                .map_err(|_| DriverError::Tls("libSQL TLS handshake failed".into()))
        };
        let (stream, details) = trace.stage(ConnectStage::Tls, handshake).await?;
        trace.tls = Some(TlsStatus::Encrypted(details));
        Ok(Box::new(stream))
    }

    pub(crate) fn redirect(&mut self, base: &str) -> Result<(), DriverError> {
        let next = Url::parse(base).map_err(|_| crate::error::protocol())?;
        if next.origin() != self.url.origin()
            || !next.username().is_empty()
            || next.password().is_some()
            || next.query().is_some()
            || next.fragment().is_some()
        {
            return Err(DriverError::Unsupported("libSQL base_url changed origin or contained credentials; reconnect to that host explicitly".into()));
        }
        self.url = next;
        Ok(())
    }
}
