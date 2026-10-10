// SPDX-License-Identifier: Apache-2.0
//! ADR-0010 wire contract, independent of Tauri and native database libraries.
mod allocation;
mod arrow;
mod arrow_decode;
mod arrow_validate;
mod bounded;
mod connect;
mod control;
mod decoder;
mod driver;
mod facet_catalog;
mod facet_codec;
mod facet_inspection;
mod facet_schema;
mod facets;
mod framing;
mod handshake;
mod lifecycle;
mod memory;
mod queue;
mod replies;
mod split;

pub use arrow::{ArrowEncoder, EncodedBatch};
pub use arrow_decode::ArrowDecoder;
pub use connect::{OwnedConnect, WireEndpoint, WireFile, WireSecret, WireTls};
pub use control::{CatalogKind, Control, FlowKey, Grant, MessageKind};
pub use decoder::FrameDecoder;
pub use driver::HostDriverId;
pub use facet_catalog::*;
pub use facet_codec::FACET_LIMIT;
pub use facet_inspection::*;
pub use facet_schema::*;
pub use facets::{CatalogRequest, EditorRequest, FacetReply, WireEdit, WireRowEditInfo};
pub use framing::{Frame, Header, Lane, read_frame, write_frame};
pub use handshake::{
    Authentication, Bootstrap, Hello, LaneHello, WireCapabilities, decode_bootstrap,
    encode_bootstrap,
};
pub use lifecycle::{CreditState, RequestTracker};
pub use memory::batch_memory;
pub use queue::{ControlQueue, QueueReceiver, QueuedFrame};
pub use replies::{Connected, Facets, Reply, WireError, WireSummary, WireTrace};
pub use split::{BatchParts, split_batch};

/// Maximum postcard control payload.
pub const CONTROL_LIMIT: usize = 1024 * 1024;
/// Normal queued control-byte limit, including framing.
pub const CONTROL_QUEUE_BYTES: usize = 4 * 1024 * 1024;
/// Reserved priority queue bytes.
pub const PRIORITY_QUEUE_BYTES: usize = 64 * 1024;
/// Maximum outstanding normal commands/replies.
pub const QUEUE_CAPACITY: usize = 32;
/// Maximum data stream fragment.
pub const FRAGMENT_LIMIT: usize = 64 * 1024;
/// Default encoded and decoded batch credit.
pub const BATCH_LIMIT: usize = 4 * 1024 * 1024;
/// Exclusive single-row encoded and decoded ceiling.
pub const LARGE_LIMIT: usize = 64 * 1024 * 1024;
/// Fixed wire protocol version.
pub const PROTOCOL: (u16, u16) = (1, 0);
/// Positional DTO layout fingerprint; revision requires an explicit protocol change.
pub const SCHEMA: &str = "openrows-host-v1-20261010-5";
/// App/helper lockstep source and dependency identity.
pub const BUILD: &str = env!("OPENROWS_HOST_BUILD");

/// Bounded failures contain no wire payload or credentials.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProtocolError {
    /// An incomplete frame or Arrow message.
    #[error("truncated host frame")]
    Truncated,
    /// An encoded, decoded or queue budget was exceeded.
    #[error("host frame exceeds its limit")]
    Limit,
    /// A required message kind is unknown.
    #[error("unknown host message kind")]
    UnknownKind,
    /// Malformed postcard, header or Arrow metadata.
    #[error("invalid host frame")]
    Invalid,
    /// A stale, duplicated or unordered operation.
    #[error("stale host operation")]
    Stale,
    /// Lockstep protocol, capabilities or helper build mismatch.
    #[error("incompatible driver host")]
    Incompatible,
    /// Missing, wrong or replayed authentication.
    #[error("driver host authentication refused")]
    Authentication,
    /// The local transport ended or failed.
    #[error("driver host transport ended")]
    Transport,
}

impl From<ProtocolError> for openrows_driver_api::HostFailure {
    fn from(error: ProtocolError) -> Self {
        match error {
            ProtocolError::Incompatible => Self::Incompatible,
            ProtocolError::Transport => Self::Exited,
            ProtocolError::Truncated
            | ProtocolError::Limit
            | ProtocolError::UnknownKind
            | ProtocolError::Invalid
            | ProtocolError::Stale
            | ProtocolError::Authentication => Self::Protocol,
        }
    }
}
