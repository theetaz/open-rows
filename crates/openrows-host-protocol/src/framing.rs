// SPDX-License-Identifier: Apache-2.0
//! Length checks precede body allocation, even on fragmented transports.
use crate::{CONTROL_LIMIT, Control, FRAGMENT_LIMIT, MessageKind, PROTOCOL, ProtocolError};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

/// Fixed header size after the four-byte length prefix.
pub const HEADER_BYTES: usize = 32;
/// Authenticated transport lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Lane {
    /// Duplex control.
    Control,
    /// Host-to-parent Arrow stream.
    Data,
}
/// Every frame's fixed identity, independent of postcard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    /// Explicit required kind (100 for Arrow fragments).
    pub kind: MessageKind,
    /// Child generation.
    pub generation: u64,
    /// Request ID (zero reserved for handshake/unsolicited health).
    pub request: u64,
    /// Result ordinal.
    pub result: u32,
    /// Fragment sequence.
    pub sequence: u64,
}
impl Header {
    /// Header for a control operation.
    #[must_use]
    pub fn control(generation: u64, request: u64, control: &Control) -> Self {
        Self {
            kind: control.kind(),
            generation,
            request,
            result: 0,
            sequence: 0,
        }
    }
    fn encode(self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0; HEADER_BYTES];
        bytes[0..2].copy_from_slice(&PROTOCOL.0.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.kind.0.to_le_bytes());
        bytes[4..12].copy_from_slice(&self.generation.to_le_bytes());
        bytes[12..20].copy_from_slice(&self.request.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.result.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.sequence.to_le_bytes());
        bytes
    }
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let integer =
            |range: std::ops::Range<usize>| bytes.get(range).ok_or(ProtocolError::Truncated);
        let version = u16::from_le_bytes(
            integer(0..2)?
                .try_into()
                .map_err(|_| ProtocolError::Invalid)?,
        );
        if version != PROTOCOL.0 {
            return Err(ProtocolError::Incompatible);
        }
        let header = Self {
            kind: MessageKind(u16::from_le_bytes(
                integer(2..4)?
                    .try_into()
                    .map_err(|_| ProtocolError::Invalid)?,
            )),
            generation: u64::from_le_bytes(
                integer(4..12)?
                    .try_into()
                    .map_err(|_| ProtocolError::Invalid)?,
            ),
            request: u64::from_le_bytes(
                integer(12..20)?
                    .try_into()
                    .map_err(|_| ProtocolError::Invalid)?,
            ),
            result: u32::from_le_bytes(
                integer(20..24)?
                    .try_into()
                    .map_err(|_| ProtocolError::Invalid)?,
            ),
            sequence: u64::from_le_bytes(
                integer(24..32)?
                    .try_into()
                    .map_err(|_| ProtocolError::Invalid)?,
            ),
        };
        header.kind.validate()?;
        Ok(header)
    }
}
/// Frame body zeroizes credentials on every success/error/drop path.
pub struct Frame {
    /// Fixed routing header.
    pub header: Header,
    /// Bounded body.
    pub body: Zeroizing<Vec<u8>>,
}
impl std::fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Frame")
            .field("header", &self.header)
            .field("body_bytes", &self.body.len())
            .finish()
    }
}
impl Frame {
    /// Encode a postcard control DTO under the control-body ceiling.
    /// # Errors
    /// Oversized or invalid DTO.
    pub fn control(
        generation: u64,
        request: u64,
        control: &Control,
    ) -> Result<Self, ProtocolError> {
        let size = postcard::serialize_with_flavor(control, postcard::ser_flavors::Size::default())
            .map_err(|_| ProtocolError::Invalid)?;
        if size > CONTROL_LIMIT {
            return Err(ProtocolError::Limit);
        }
        let mut buffer = Zeroizing::new(vec![0; size]);
        postcard::to_slice(control, &mut buffer).map_err(|_| ProtocolError::Invalid)?;
        Ok(Self {
            header: Header::control(generation, request, control),
            body: buffer,
        })
    }
    /// Decode only a validated control kind; trailing bytes are rejected.
    /// # Errors
    /// Malformed body, oversized collection, unknown or inconsistent kind.
    pub fn decode_control(&self) -> Result<Control, ProtocolError> {
        if self.body.len() > CONTROL_LIMIT || self.header.kind.0 == 100 {
            return Err(ProtocolError::Invalid);
        }
        let (control, rest): (Control, _) =
            postcard::take_from_bytes(&self.body).map_err(|_| ProtocolError::Invalid)?;
        if !rest.is_empty() || control.kind() != self.header.kind {
            return Err(ProtocolError::Invalid);
        }
        Ok(control)
    }
    /// Encode an entire bounded frame.
    /// # Errors
    /// Body exceeds its lane ceiling.
    pub fn encode(&self, lane: Lane) -> Result<Zeroizing<Vec<u8>>, ProtocolError> {
        validate_length(self.body.len() + HEADER_BYTES, lane)?;
        let mut bytes = Zeroizing::new(Vec::with_capacity(self.body.len() + HEADER_BYTES + 4));
        let length =
            u32::try_from(self.body.len() + HEADER_BYTES).map_err(|_| ProtocolError::Limit)?;
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(&self.header.encode());
        bytes.extend_from_slice(&self.body);
        Ok(bytes)
    }
}
pub(crate) fn validate_length(length: usize, lane: Lane) -> Result<(), ProtocolError> {
    let maximum = match lane {
        Lane::Control => CONTROL_LIMIT,
        Lane::Data => FRAGMENT_LIMIT,
    };
    if length < HEADER_BYTES {
        Err(ProtocolError::Invalid)
    } else if length > maximum + HEADER_BYTES {
        Err(ProtocolError::Limit)
    } else {
        Ok(())
    }
}
pub(crate) fn validate_lane(header: Header, lane: Lane) -> Result<(), ProtocolError> {
    let valid = match lane {
        Lane::Data => matches!(header.kind.0, 1 | 100),
        Lane::Control => header.kind.0 != 100,
    };
    if valid {
        Ok(())
    } else {
        Err(ProtocolError::Invalid)
    }
}
/// Read the fixed prefix/header before allocating the checked body.
/// # Errors
/// Bounded framing error, EOF or local transport failure.
pub async fn read_frame(
    reader: &mut (impl AsyncRead + Unpin),
    lane: Lane,
) -> Result<Frame, ProtocolError> {
    let mut prefix = [0; 4];
    let count = reader
        .read(&mut prefix)
        .await
        .map_err(|error| io_error(&error))?;
    if count == 0 {
        return Err(ProtocolError::Transport);
    }
    reader
        .read_exact(&mut prefix[count..])
        .await
        .map_err(|error| io_error(&error))?;
    let length = u32::from_le_bytes(prefix) as usize;
    validate_length(length, lane)?;
    let mut bytes = [0; HEADER_BYTES];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|error| io_error(&error))?;
    let header = Header::decode(&bytes)?;
    validate_lane(header, lane)?;
    let mut body = Zeroizing::new(vec![0; length - HEADER_BYTES]);
    reader
        .read_exact(&mut body)
        .await
        .map_err(|error| io_error(&error))?;
    Ok(Frame { header, body })
}
/// Write a frame without constructing a second copy of its payload.
/// # Errors
/// Invalid lane/size or ended transport.
pub async fn write_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    frame: &Frame,
    lane: Lane,
) -> Result<(), ProtocolError> {
    validate_length(frame.body.len() + HEADER_BYTES, lane)?;
    validate_lane(frame.header, lane)?;
    let length =
        u32::try_from(frame.body.len() + HEADER_BYTES).map_err(|_| ProtocolError::Limit)?;
    let mut prefix = [0; HEADER_BYTES + 4];
    prefix[..4].copy_from_slice(&length.to_le_bytes());
    prefix[4..].copy_from_slice(&frame.header.encode());
    writer
        .write_all(&prefix)
        .await
        .map_err(|error| io_error(&error))?;
    writer
        .write_all(&frame.body)
        .await
        .map_err(|error| io_error(&error))?;
    Ok(())
}
fn io_error(error: &std::io::Error) -> ProtocolError {
    if error.kind() == std::io::ErrorKind::UnexpectedEof {
        ProtocolError::Truncated
    } else {
        ProtocolError::Transport
    }
}
