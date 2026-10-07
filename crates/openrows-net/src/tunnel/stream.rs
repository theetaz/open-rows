// SPDX-License-Identifier: Apache-2.0
//! In-process forwarded byte stream handed to drivers (docs/12 §4.2).

use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use russh::{ChannelStream, client::Msg};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// A byte stream to the destination, forwarded through the last hop.
pub struct TunnelStream(pub(super) ChannelStream<Msg>);

impl AsyncRead for TunnelStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(context, buffer)
    }
}

impl AsyncWrite for TunnelStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(context, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(context)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(context)
    }
}
