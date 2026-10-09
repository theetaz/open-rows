// SPDX-License-Identifier: Apache-2.0
//! TLS for TDS 7.x (MS-TDS 2.2.6.5): the handshake records travel inside
//! `PRELOGIN` packets; once it is done, TLS records flow directly on the
//! stream. [`PreloginFraming`] sits between rustls and the socket and
//! switches to pass-through after the handshake.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::packet::{HEADER_LEN, Header, INITIAL_PACKET_SIZE, PacketType, frame};

/// Wraps handshake bytes in `PRELOGIN` packets until [`Self::pass_through`].
pub(crate) struct PreloginFraming<S> {
    inner: S,
    framing: bool,
    /// Handshake bytes written since the last flush.
    pending: Vec<u8>,
    /// Framed packets being written, and how much of them went out.
    out: Vec<u8>,
    written: usize,
    packet_id: u8,
    header: [u8; HEADER_LEN],
    header_len: usize,
    /// Payload bytes of the current inbound packet not read yet.
    payload_left: usize,
}

impl<S> PreloginFraming<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self {
            inner,
            framing: true,
            pending: Vec::new(),
            out: Vec::new(),
            written: 0,
            packet_id: 1,
            header: [0; HEADER_LEN],
            header_len: 0,
            payload_left: 0,
        }
    }

    /// The handshake is done: pass bytes through unchanged from now on.
    pub(crate) fn pass_through(&mut self) {
        self.framing = false;
    }

    pub(crate) fn into_inner(self) -> S {
        self.inner
    }
}

impl<S: AsyncRead + Unpin> PreloginFraming<S> {
    /// Read the next packet header; `false` at end of stream.
    fn poll_header(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<bool>> {
        while self.header_len < HEADER_LEN {
            let mut buf = ReadBuf::new(&mut self.header[self.header_len..]);
            ready!(Pin::new(&mut self.inner).poll_read(cx, &mut buf))?;
            let read = buf.filled().len();
            if read == 0 {
                return Poll::Ready(Ok(false));
            }
            self.header_len += read;
        }
        self.header_len = 0;
        let header = Header::parse(self.header)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        self.payload_left = header.payload;
        Poll::Ready(Ok(true))
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for PreloginFraming<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if !this.framing {
            return Pin::new(&mut this.inner).poll_read(cx, buf);
        }
        while this.payload_left == 0 {
            if !ready!(this.poll_header(cx))? {
                return Poll::Ready(Ok(()));
            }
        }
        let mut chunk = [0_u8; INITIAL_PACKET_SIZE];
        let limit = this.payload_left.min(buf.remaining()).min(chunk.len());
        let mut limited = ReadBuf::new(&mut chunk[..limit]);
        ready!(Pin::new(&mut this.inner).poll_read(cx, &mut limited))?;
        this.payload_left -= limited.filled().len();
        buf.put_slice(limited.filled());
        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for PreloginFraming<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if !this.framing {
            return Pin::new(&mut this.inner).poll_write(cx, buf);
        }
        this.pending.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.written == this.out.len() && !this.pending.is_empty() {
            this.out = frame(
                PacketType::PreLogin,
                &this.pending,
                INITIAL_PACKET_SIZE,
                &mut this.packet_id,
            );
            this.written = 0;
            this.pending.clear();
        }
        while this.written < this.out.len() {
            let sent = ready!(Pin::new(&mut this.inner).poll_write(cx, &this.out[this.written..]))?;
            if sent == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
            this.written += sent;
        }
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

    #[tokio::test]
    async fn handshake_bytes_are_framed_then_pass_through() -> io::Result<()> {
        let (client, mut server) = duplex(64 * 1024);
        let mut framing = PreloginFraming::new(client);
        framing.write_all(b"hello").await?;
        framing.flush().await?;
        let mut packet = [0_u8; HEADER_LEN + 5];
        server.read_exact(&mut packet).await?;
        assert_eq!(&packet[..4], &[0x12, 1, 0, 13]);
        assert_eq!(&packet[HEADER_LEN..], b"hello");
        server
            .write_all(&[0x04, 1, 0, 11, 0, 0, 1, 0, b'a', b'b', b'c'])
            .await?;
        let mut reply = [0_u8; 3];
        framing.read_exact(&mut reply).await?;
        assert_eq!(&reply, b"abc");
        framing.pass_through();
        framing.write_all(b"raw").await?;
        framing.flush().await?;
        let mut raw = [0_u8; 3];
        server.read_exact(&mut raw).await?;
        assert_eq!(&raw, b"raw");
        Ok(())
    }
}
