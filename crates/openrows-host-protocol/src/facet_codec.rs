// SPDX-License-Identifier: Apache-2.0
//! Bounded facet serialization, shared by chunk producer and assembler.
use crate::{FacetReply, ProtocolError};
use zeroize::Zeroizing;
/// Aggregate encoded facet ceiling, charged within each active host reservation.
pub const FACET_LIMIT: usize = 16 * 1024 * 1024;
impl FacetReply {
    /// Encode only after measuring size; nested collection limits are checked too.
    /// # Errors
    /// Unsupported aggregate size or nested collection.
    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>, ProtocolError> {
        let size = postcard::serialize_with_flavor(self, postcard::ser_flavors::Size::default())
            .map_err(|_| ProtocolError::Invalid)?;
        if size > FACET_LIMIT {
            return Err(ProtocolError::Limit);
        }
        let mut bytes = Zeroizing::new(vec![0; size]);
        postcard::to_slice(self, &mut bytes).map_err(|_| ProtocolError::Invalid)?;
        Self::decode(&bytes)?;
        Ok(bytes)
    }
    /// Decode a complete bounded facet, rejecting trailing data and oversized lists.
    /// # Errors
    /// Invalid payload or aggregate size limit.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > FACET_LIMIT {
            return Err(ProtocolError::Limit);
        }
        let _scope = crate::allocation::Scope::begin();
        let (value, rest) = postcard::take_from_bytes(bytes).map_err(|_| ProtocolError::Invalid)?;
        if !rest.is_empty() {
            return Err(ProtocolError::Invalid);
        }
        Ok(value)
    }
}
