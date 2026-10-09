// SPDX-License-Identifier: Apache-2.0
//! One connection's two directions. The session actor owns the inbound
//! side and parses tokens from a growing buffer; the outbound side is
//! shared with the cancel handle, which may send an attention signal while
//! a request runs (MS-TDS 2.2.1.7). Both sides lock the outbound state, so
//! an attention is only ever sent for a request that is in flight.

use std::sync::Arc;

use openrows_driver_api::DriverError;
use openrows_net::BoxStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::Mutex;

use super::packet::{HEADER_LEN, Header, INITIAL_PACKET_SIZE, PacketType, frame};

/// Buffer bytes kept before already-parsed bytes are dropped.
const COMPACT_AFTER: usize = 64 * 1024;

/// The writing half and the state of the request on the wire.
pub(crate) struct Outbound {
    half: WriteHalf<BoxStream>,
    packet_id: u8,
    packet_size: usize,
    in_flight: bool,
    attention: bool,
}

/// The outbound side, shared with the cancel handle.
pub(crate) type SharedOutbound = Arc<Mutex<Outbound>>;

impl Outbound {
    async fn write(&mut self, kind: PacketType, payload: &[u8]) -> Result<(), DriverError> {
        let packets = frame(kind, payload, self.packet_size, &mut self.packet_id);
        self.half.write_all(&packets).await?;
        self.half.flush().await?;
        Ok(())
    }

    /// Send an attention signal if a request is in flight and none was
    /// sent for it yet.
    pub(crate) async fn attention(&mut self) -> Result<(), DriverError> {
        if !self.in_flight || self.attention {
            return Ok(());
        }
        self.attention = true;
        self.write(PacketType::Attention, &[]).await
    }
}

/// The reading half and the bytes of the current message.
pub(crate) struct Inbound {
    half: ReadHalf<BoxStream>,
    buf: Vec<u8>,
    pos: usize,
    /// The end-of-message packet of the current message has been read.
    message_done: bool,
}

impl Inbound {
    /// Unparsed bytes of the current message.
    pub(crate) fn bytes(&self) -> &[u8] {
        self.buf.get(self.pos..).unwrap_or_default()
    }

    /// Mark `len` bytes as parsed.
    pub(crate) fn consume(&mut self, len: usize) {
        self.pos = (self.pos + len).min(self.buf.len());
    }

    /// Expect the next message (a reply after a request, or the
    /// acknowledgement of an attention).
    pub(crate) fn next_message(&mut self) {
        self.message_done = false;
    }

    /// Read packets until at least twice the unparsed bytes (and one more
    /// packet) are buffered, so a token spanning many packets is parsed a
    /// logarithmic number of times.
    ///
    /// # Errors
    /// `Network` on transport failures; `Protocol` when the message ends
    /// inside a token.
    pub(crate) async fn fill(&mut self) -> Result<(), DriverError> {
        let wanted = self.bytes().len().saturating_mul(2).max(1);
        self.compact();
        loop {
            if self.message_done {
                return Err(DriverError::Protocol(
                    "the server's reply ended inside a token".to_owned(),
                ));
            }
            self.read_packet().await?;
            if self.bytes().len() >= wanted || self.message_done {
                return Ok(());
            }
        }
    }

    async fn read_packet(&mut self) -> Result<(), DriverError> {
        let mut header = [0_u8; HEADER_LEN];
        self.half.read_exact(&mut header).await?;
        let header = Header::parse(header)?;
        let start = self.buf.len();
        self.buf.resize(start + header.payload, 0);
        self.half.read_exact(&mut self.buf[start..]).await?;
        self.message_done = header.last;
        Ok(())
    }

    fn compact(&mut self) {
        if self.pos == self.buf.len() {
            self.buf.clear();
            self.pos = 0;
        } else if self.pos > COMPACT_AFTER {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }
    }
}

/// Both halves of one connection.
pub(crate) struct Wire {
    pub(crate) inbound: Inbound,
    pub(crate) outbound: SharedOutbound,
}

impl Wire {
    pub(crate) fn new(stream: BoxStream) -> Self {
        let (read, write) = tokio::io::split(stream);
        Self {
            inbound: Inbound {
                half: read,
                buf: Vec::new(),
                pos: 0,
                message_done: true,
            },
            outbound: Arc::new(Mutex::new(Outbound {
                half: write,
                packet_id: 1,
                packet_size: INITIAL_PACKET_SIZE,
                in_flight: false,
                attention: false,
            })),
        }
    }

    /// Send a request and expect its reply.
    ///
    /// # Errors
    /// `Network` when the request cannot be written.
    pub(crate) async fn send(
        &mut self,
        kind: PacketType,
        payload: &[u8],
    ) -> Result<(), DriverError> {
        let mut outbound = self.outbound.lock().await;
        outbound.in_flight = true;
        outbound.attention = false;
        self.inbound.next_message();
        outbound.write(kind, payload).await
    }

    /// Expect a reply to a message sent before the wire existed (a login
    /// sent inside the TLS handshake).
    pub(crate) fn expect_reply(&mut self) {
        self.inbound.next_message();
    }

    /// The request's reply has ended; true when an attention was sent for
    /// it, whose acknowledgement must then be read.
    pub(crate) async fn finish_request(&mut self) -> bool {
        let mut outbound = self.outbound.lock().await;
        outbound.in_flight = false;
        std::mem::take(&mut outbound.attention)
    }

    /// Use the packet size the server chose.
    pub(crate) async fn set_packet_size(&mut self, size: usize) {
        self.outbound.lock().await.packet_size = size;
    }

    /// Close the writing direction; the server then ends the session.
    pub(crate) async fn shutdown(&mut self) -> Result<(), DriverError> {
        self.outbound.lock().await.half.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn replies_are_buffered_across_packets_and_attention_needs_a_request()
    -> Result<(), DriverError> {
        let (client, mut server) = duplex(64 * 1024);
        let mut wire = Wire::new(Box::new(client));
        wire.outbound.lock().await.attention().await?;
        wire.send(PacketType::SqlBatch, b"x").await?;
        let mut request = [0_u8; HEADER_LEN + 1];
        server.read_exact(&mut request).await?;
        assert_eq!(request[0], PacketType::SqlBatch as u8);
        server
            .write_all(&[4, 0, 0, 10, 0, 0, 1, 0, 1, 2, 4, 1, 0, 9, 0, 0, 2, 0, 3])
            .await?;
        wire.inbound.fill().await?;
        assert_eq!(wire.inbound.bytes(), &[1, 2]);
        wire.inbound.consume(1);
        wire.inbound.fill().await?;
        assert_eq!(wire.inbound.bytes(), &[2, 3]);
        wire.inbound.consume(2);
        assert!(wire.inbound.fill().await.is_err());
        wire.outbound.lock().await.attention().await?;
        let mut attention = [0_u8; HEADER_LEN];
        server.read_exact(&mut attention).await?;
        assert_eq!(attention[0], PacketType::Attention as u8);
        assert!(wire.finish_request().await);
        assert!(!wire.finish_request().await);
        Ok(())
    }
}
