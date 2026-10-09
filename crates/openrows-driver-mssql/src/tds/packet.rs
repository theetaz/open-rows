// SPDX-License-Identifier: Apache-2.0
//! TDS packets (MS-TDS 2.2.3): an 8-byte header (type, status, big-endian
//! length including the header, SPID, packet id, window) and a payload. A
//! message is one or more packets; the last carries the end-of-message bit.

use openrows_driver_api::DriverError;

/// Header length.
pub(crate) const HEADER_LEN: usize = 8;
/// Packet size used until the server's login acknowledgement says otherwise.
pub(crate) const INITIAL_PACKET_SIZE: usize = 4096;
/// Largest packet size the protocol allows.
const MAX_PACKET_SIZE: usize = 32_767;
/// Status bit of the last packet of a message.
const END_OF_MESSAGE: u8 = 0x01;

/// The message kind in a packet header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PacketType {
    /// SQL batch (`0x01`).
    SqlBatch = 0x01,
    /// Remote procedure call (`0x03`).
    Rpc = 0x03,
    /// Attention signal: cancel the running request (`0x06`).
    Attention = 0x06,
    /// `LOGIN7` (`0x10`).
    Login = 0x10,
    /// `PRELOGIN`, and the TLS handshake wrapped in it (`0x12`).
    PreLogin = 0x12,
}

/// A parsed packet header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Header {
    /// Payload bytes after the header.
    pub(crate) payload: usize,
    /// The last packet of its message.
    pub(crate) last: bool,
}

impl Header {
    /// Parse and check a header.
    ///
    /// # Errors
    /// `Protocol` for a length shorter than the header or over the maximum.
    pub(crate) fn parse(bytes: [u8; HEADER_LEN]) -> Result<Self, DriverError> {
        let length = usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
        if !(HEADER_LEN..=MAX_PACKET_SIZE + HEADER_LEN).contains(&length) {
            return Err(DriverError::Protocol(format!(
                "packet length {length} is out of range"
            )));
        }
        Ok(Self {
            payload: length - HEADER_LEN,
            last: bytes[1] & END_OF_MESSAGE != 0,
        })
    }
}

/// Split `payload` into packets of at most `packet_size` bytes, numbering
/// them from `*id` (which wraps), the last one marked end-of-message.
pub(crate) fn frame(kind: PacketType, payload: &[u8], packet_size: usize, id: &mut u8) -> Vec<u8> {
    let chunk = packet_size.clamp(512, MAX_PACKET_SIZE) - HEADER_LEN;
    let count = payload.len().div_ceil(chunk).max(1);
    let mut out = Vec::with_capacity(payload.len() + count * HEADER_LEN);
    for index in 0..count {
        let body = payload
            .get(index * chunk..((index + 1) * chunk).min(payload.len()))
            .unwrap_or_default();
        let last = index + 1 == count;
        push_header(&mut out, kind, body.len(), last, *id);
        out.extend_from_slice(body);
        *id = id.wrapping_add(1);
    }
    out
}

fn push_header(out: &mut Vec<u8>, kind: PacketType, body: usize, last: bool, id: u8) {
    // `frame` keeps every packet within `MAX_PACKET_SIZE`, so the length fits.
    let length = u16::try_from(body + HEADER_LEN).unwrap_or(u16::MAX);
    out.push(kind as u8);
    out.push(if last { END_OF_MESSAGE } else { 0 });
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(&[0, 0, id, 0]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_split_into_numbered_packets_with_one_end_marker() {
        let payload = vec![7_u8; 1000];
        let mut id = 255;
        let packets = frame(PacketType::SqlBatch, &payload, 512, &mut id);
        assert_eq!(id, 1);
        assert_eq!(packets.len(), 1000 + 2 * HEADER_LEN);
        let first: [u8; HEADER_LEN] = packets[..HEADER_LEN].try_into().unwrap_or_default();
        assert_eq!(first, [1, 0, 2, 0, 0, 0, 255, 0]);
        let header = Header::parse(first);
        assert!(matches!(
            header,
            Ok(Header {
                payload: 504,
                last: false
            })
        ));
        let last_at = 512;
        assert_eq!(packets[last_at + 1], END_OF_MESSAGE);
        assert_eq!(packets[last_at + 6], 0);
    }

    #[test]
    fn empty_messages_are_one_packet_and_bad_lengths_are_refused() {
        let mut id = 0;
        let attention = frame(PacketType::Attention, &[], 4096, &mut id);
        assert_eq!(attention, [6, 1, 0, 8, 0, 0, 0, 0]);
        assert!(Header::parse([4, 1, 0, 7, 0, 0, 0, 0]).is_err());
    }
}
