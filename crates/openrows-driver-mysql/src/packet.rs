// SPDX-License-Identifier: Apache-2.0
//! `MySQL` packet framing (the protocol's "packets" section): a 3-byte little-endian
//! length, a sequence id, and the payload; payloads of 16 MiB − 1 or more
//! continue in the next packet. Compression is never negotiated.
use std::io;

use openrows_driver_api::DriverError;
use openrows_net::BoxStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Largest payload of one physical packet.
const MAX_PAYLOAD: usize = 0xFF_FFFF;

/// A stream speaking `MySQL` packets.
pub(crate) struct Packets {
    stream: BoxStream,
    sequence: u8,
    limit: usize,
    invalid: bool,
}

impl Packets {
    pub(crate) fn new(stream: BoxStream) -> Self {
        Self {
            stream,
            sequence: 0,
            limit: usize::MAX,
            invalid: false,
        }
    }

    /// Hand the stream over (for the TLS upgrade); the sequence carries on.
    pub(crate) fn into_parts(self) -> (BoxStream, u8) {
        (self.stream, self.sequence)
    }

    /// Continue on `stream` (now encrypted) at `sequence`.
    pub(crate) fn resume(stream: BoxStream, sequence: u8) -> Self {
        Self {
            stream,
            sequence,
            limit: usize::MAX,
            invalid: false,
        }
    }

    /// Every command starts a new sequence.
    pub(crate) fn reset(&mut self) {
        self.sequence = 0;
    }

    /// Bound monitor packets before allocating payload memory.
    pub(crate) fn limit(&mut self, bytes: usize) {
        self.limit = bytes;
    }

    /// Refuse reuse after a partial response or client deadline.
    pub(crate) fn invalidate(&mut self) {
        self.invalid = true;
    }

    /// Read one logical packet.
    ///
    /// # Errors
    /// `Network` for transport failures, `Protocol` for a sequence mismatch.
    pub(crate) async fn read(&mut self) -> Result<Vec<u8>, DriverError> {
        let mut payload = Vec::new();
        loop {
            let mut header = [0_u8; 4];
            self.stream.read_exact(&mut header).await?;
            let length =
                usize::from(header[0]) | usize::from(header[1]) << 8 | usize::from(header[2]) << 16;
            if header[3] != self.sequence {
                return Err(DriverError::Protocol(format!(
                    "packet sequence {} where {} was expected",
                    header[3], self.sequence
                )));
            }
            self.sequence = self.sequence.wrapping_add(1);
            if payload.len().saturating_add(length) > self.limit {
                self.invalidate();
                return Err(DriverError::Protocol("monitor frame limit exceeded".into()));
            }
            let start = payload.len();
            payload.resize(start + length, 0);
            self.stream.read_exact(&mut payload[start..]).await?;
            if length < MAX_PAYLOAD {
                return Ok(payload);
            }
        }
    }

    /// Write `payload` as one logical packet and flush.
    ///
    /// # Errors
    /// `Network` for transport failures.
    pub(crate) async fn write(&mut self, payload: &[u8]) -> Result<(), DriverError> {
        if self.invalid {
            return Err(DriverError::Protocol("monitor transport is invalid".into()));
        }
        let mut framed = Vec::with_capacity(payload.len() + 8);
        let mut offset = 0;
        loop {
            let end = (offset + MAX_PAYLOAD).min(payload.len());
            self.frame(&mut framed, payload.get(offset..end).unwrap_or_default())?;
            // A chunk shorter than the maximum (possibly empty) ends the packet.
            if end - offset < MAX_PAYLOAD {
                break;
            }
            offset = end;
        }
        self.stream.write_all(&framed).await?;
        self.stream.flush().await?;
        Ok(())
    }

    fn frame(&mut self, out: &mut Vec<u8>, chunk: &[u8]) -> Result<(), DriverError> {
        let length = u32::try_from(chunk.len())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "packet too large"))?;
        out.extend_from_slice(&length.to_le_bytes()[..3]);
        out.push(self.sequence);
        out.extend_from_slice(chunk);
        self.sequence = self.sequence.wrapping_add(1);
        Ok(())
    }

    /// Close the transport (after `COM_QUIT`).
    pub(crate) async fn shutdown(&mut self) -> Result<(), DriverError> {
        self.stream.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn packets_round_trip_with_sequence_ids() -> Result<(), DriverError> {
        let (near, far) = tokio::io::duplex(1024);
        let mut client = Packets::new(Box::new(near));
        let mut server = Packets::new(Box::new(far));
        client.write(b"\x03SELECT 1").await?;
        assert_eq!(server.read().await?, b"\x03SELECT 1");
        server.write(b"ok").await?;
        assert_eq!(client.read().await?, b"ok");
        client.reset();
        server.reset();
        client.write(&[]).await?;
        assert_eq!(server.read().await?, Vec::<u8>::new());
        Ok(())
    }

    #[tokio::test]
    async fn a_wrong_sequence_id_is_a_protocol_error() -> Result<(), DriverError> {
        let (near, mut far) = tokio::io::duplex(64);
        let mut client = Packets::new(Box::new(near));
        far.write_all(&[1, 0, 0, 7, 0]).await?;
        assert!(matches!(client.read().await, Err(DriverError::Protocol(_))));
        Ok(())
    }
    #[tokio::test]
    async fn monitor_packet_limit_rejects_header_before_payload_read() -> Result<(), DriverError> {
        let (near, mut far) = tokio::io::duplex(64);
        let mut client = Packets::new(Box::new(near));
        client.limit(16);
        far.write_all(&[17, 0, 0, 0]).await?;
        assert!(
            matches!(client.read().await, Err(DriverError::Protocol(reason)) if reason == "monitor frame limit exceeded")
        );
        assert!(client.write(b"query").await.is_err());
        Ok(())
    }
}
