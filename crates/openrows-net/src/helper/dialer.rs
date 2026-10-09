// SPDX-License-Identifier: Apache-2.0
//! Routed TCP streams retain a lease and report helper failures as network errors.
use super::Helper;
use crate::{BoxFuture, BoxStream, Dialer};
use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};

/// Dial a helper's loopback endpoint while preserving the driver's logical host.
#[derive(Clone)]
pub struct HelperDialer(Arc<Helper>);
impl HelperDialer {
    /// Keep this shared helper alive until every stream/dialer is dropped.
    #[must_use]
    pub fn new(helper: Arc<Helper>) -> Self {
        Self(helper)
    }
}
impl Dialer for HelperDialer {
    fn dial(&self) -> BoxFuture<'_, io::Result<BoxStream>> {
        Box::pin(async move {
            self.0.ready().await.map_err(io::Error::other)?;
            let stream = TcpStream::connect(self.0.endpoint())
                .await
                .map_err(|_| io::Error::other(self.0.failure()))?;
            Ok(Box::new(Forwarded {
                stream,
                helper: Arc::clone(&self.0),
            }) as BoxStream)
        })
    }
    fn describe(&self) -> String {
        self.0.spec.hop.describe()
    }
}
struct Forwarded {
    stream: TcpStream,
    helper: Arc<Helper>,
}
impl AsyncRead for Forwarded {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buffer.filled().len();
        let has_space = buffer.remaining() > 0;
        match Pin::new(&mut self.stream).poll_read(context, buffer) {
            Poll::Ready(Ok(())) if has_space && buffer.filled().len() == before => {
                Poll::Ready(Err(io::Error::other(self.helper.failure())))
            }
            Poll::Ready(Err(_)) => Poll::Ready(Err(io::Error::other(self.helper.failure()))),
            other => other,
        }
    }
}
impl AsyncWrite for Forwarded {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(context, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(context)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(context)
    }
}
