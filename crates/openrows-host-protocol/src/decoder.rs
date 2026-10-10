// SPDX-License-Identifier: Apache-2.0
//! Incremental framing preserves bounded fragmented input without async I/O.
use crate::{
    Frame, Header, Lane, ProtocolError,
    framing::{HEADER_BYTES, validate_lane, validate_length},
};
use zeroize::{Zeroize, Zeroizing};

/// Incremental bounded decoder for tests and non-Tokio consumers.
pub struct FrameDecoder {
    lane: Lane,
    buffered: Zeroizing<Vec<u8>>,
    expected: Option<usize>,
}
impl FrameDecoder {
    /// Start a decoder for one authenticated lane.
    #[must_use]
    pub fn new(lane: Lane) -> Self {
        Self {
            lane,
            buffered: Zeroizing::new(Vec::new()),
            expected: None,
        }
    }
    /// Feed arbitrary fragments; at most 32 frames are returned per call.
    /// # Errors
    /// Invalid, unknown or oversized frame.
    pub fn push(&mut self, mut bytes: &[u8]) -> Result<Vec<Frame>, ProtocolError> {
        let mut frames = Vec::new();
        while !bytes.is_empty() {
            let target = self.expected.unwrap_or(4);
            let take = bytes.len().min(target - self.buffered.len());
            self.buffered.extend_from_slice(&bytes[..take]);
            bytes = &bytes[take..];
            if self.buffered.len() < target {
                continue;
            }
            if self.expected.is_none() {
                let length = u32::from_le_bytes(
                    self.buffered[..4]
                        .try_into()
                        .map_err(|_| ProtocolError::Invalid)?,
                ) as usize;
                validate_length(length, self.lane)?;
                self.expected = Some(length + 4);
                continue;
            }
            let header = Header::decode(&self.buffered[4..4 + HEADER_BYTES])?;
            validate_lane(header, self.lane)?;
            frames.push(Frame {
                header,
                body: Zeroizing::new(self.buffered[4 + HEADER_BYTES..].to_vec()),
            });
            self.buffered.zeroize();
            self.expected = None;
            if frames.len() > crate::QUEUE_CAPACITY {
                return Err(ProtocolError::Limit);
            }
        }
        Ok(frames)
    }
    /// Signal EOF; a partial frame is an explicit failure.
    /// # Errors
    /// Truncated prefix/header/body.
    pub fn finish(&self) -> Result<(), ProtocolError> {
        if self.buffered.is_empty() {
            Ok(())
        } else {
            Err(ProtocolError::Truncated)
        }
    }
}
