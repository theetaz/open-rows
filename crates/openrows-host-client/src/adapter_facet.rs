// SPDX-License-Identifier: Apache-2.0
//! Bounded, ordered facet assembly acknowledges one fragment at a time.
use openrows_host_protocol::{FACET_LIMIT, FacetReply};
use zeroize::Zeroizing;
#[derive(Default)]
pub(crate) struct FacetChunks {
    sequence: u64,
    bytes: Zeroizing<Vec<u8>>,
}
impl FacetChunks {
    pub(crate) fn push(&mut self, sequence: u64, bytes: &[u8]) -> Result<(), ()> {
        if sequence != self.sequence + 1
            || bytes.is_empty()
            || bytes.len() > openrows_host_protocol::FRAGMENT_LIMIT
            || self.bytes.len().saturating_add(bytes.len()) > FACET_LIMIT
        {
            return Err(());
        }
        self.bytes.extend_from_slice(bytes);
        self.sequence = sequence;
        Ok(())
    }
    pub(crate) fn finish(self, sequence: u64) -> Result<FacetReply, ()> {
        if sequence != self.sequence || sequence == 0 {
            return Err(());
        }
        FacetReply::decode(&self.bytes).map_err(|_| ())
    }
    pub(crate) fn started(&self) -> bool {
        self.sequence != 0
    }
}
