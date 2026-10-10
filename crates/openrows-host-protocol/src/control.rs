// SPDX-License-Identifier: Apache-2.0
//! Explicit message IDs avoid depending on Rust enum declaration order.
use crate::{Hello, LaneHello, OwnedConnect, ProtocolError, Reply, WireError};
use serde::{Deserialize, Serialize};

/// An execution, result and batch identity; late flow cannot resume another query.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct FlowKey {
    /// Monotonic request ID.
    pub request: u64,
    /// One-based result ordinal.
    pub result: u32,
    /// One-based batch sequence.
    pub batch: u64,
}

/// Checked one-batch admission, issued only after parent memory reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Grant {
    /// Exact operation and batch.
    pub key: FlowKey,
    /// Encoded stream-byte ceiling.
    pub encoded: u32,
    /// Decoded Arrow-memory ceiling.
    pub decoded: u32,
    /// Exclusive parent-wide large-row reservation is held.
    pub exclusive: bool,
}

/// Catalog placeholder operations, returning Unsupported until the facet adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CatalogKind {
    /// List databases.
    Databases,
    /// List schemas.
    Schemas,
    /// List schema objects.
    Objects,
    /// Bounded name index.
    SearchIndex,
    /// Table columns.
    Columns,
    /// Table structure.
    Structure,
    /// Table detail.
    Detail,
    /// Schema digest.
    Fingerprint,
    /// Schema diagram.
    Diagram,
    /// Installed extensions.
    Extensions,
    /// Roles and memberships.
    Roles,
    /// Scoped grants.
    Grants,
    /// Object detail.
    ObjectDetail,
    /// Routine definition.
    RoutineDefinition,
}

/// Versioned postcard bodies; credentials exist only in authenticated Connect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Control {
    /// Authenticate one transport, before other messages.
    Authenticate(LaneHello),
    /// Parent's lockstep contract.
    Hello(Hello),
    /// Host's verified contract.
    Ready(Hello),
    /// Open exactly one connection.
    Connect(OwnedConnect),
    /// Execute SQL without bound values.
    Execute {
        /// Statement text.
        sql: String,
    },
    /// Execute with bound text/NULL separate from SQL.
    ExecuteBound {
        /// Statement text.
        sql: String,
        /// Positional values.
        #[serde(deserialize_with = "crate::bounded::list")]
        params: Vec<Option<String>>,
    },
    /// Worker ping.
    Ping,
    /// Interrupt only this currently running request.
    Cancel {
        /// Execution ID.
        target: u64,
    },
    /// Native interrupt was requested, not execution completion.
    CancelAck {
        /// Execution ID.
        target: u64,
        /// Native interrupt requested; false for queued/already-completed operations.
        requested: bool,
    },
    /// Revoke future credit.
    Pause(FlowKey),
    /// Final already-authorized fragment sequence.
    Paused {
        /// Execution identity.
        key: FlowKey,
        /// Last authorized fragment.
        sequence: u64,
    },
    /// Resume with fresh credit.
    Resume(Grant),
    /// Admit precisely one batch.
    Grant(Grant),
    /// Sink accepted the authorized batch.
    Consumed(FlowKey),
    /// Stop rows and drain, distinct from Cancel.
    Stop(FlowKey),
    /// Stop worker and exit within the close grace.
    Close,
    /// Close received independently of worker progress.
    CloseAck,
    /// Control runtime health/probe, zero ID for unsolicited beats.
    Heartbeat,
    /// Normal operation receipt.
    Accepted,
    /// Exactly one terminal success.
    Reply(Reply),
    /// Exactly one terminal typed failure.
    Error(WireError),
    /// Result begins; schema follows on the data lane.
    ResultBegin,
    /// Result ended at this last fragment.
    ResultEnd {
        /// Final fragment sequence.
        sequence: u64,
    },
    /// Bounded prepared batch waits for admission.
    BatchReady(Grant),
    /// Catalog request placeholder with bounded page size.
    Catalog {
        /// Method.
        kind: CatalogKind,
        /// Page size.
        limit: u32,
    },
    /// Editor placeholder, never advertised before implementation.
    Editor,
    /// Admin placeholder.
    Admin,
    /// Bulk placeholder.
    Bulk,
}

/// Stable, explicit wire IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageKind(pub u16);

impl Control {
    /// Stable frame kind for this DTO.
    #[must_use]
    pub fn kind(&self) -> MessageKind {
        MessageKind(match self {
            Self::Authenticate(_) => 1,
            Self::Hello(_) => 2,
            Self::Ready(_) => 3,
            Self::Connect(_) => 10,
            Self::Execute { .. } => 11,
            Self::ExecuteBound { .. } => 12,
            Self::Ping => 13,
            Self::Catalog { .. } => 14,
            Self::Editor => 15,
            Self::Admin => 16,
            Self::Bulk => 17,
            Self::Cancel { .. } => 20,
            Self::CancelAck { .. } => 21,
            Self::Pause(_) => 22,
            Self::Paused { .. } => 23,
            Self::Resume(_) => 24,
            Self::Grant(_) => 25,
            Self::Consumed(_) => 26,
            Self::Stop(_) => 27,
            Self::Close => 28,
            Self::CloseAck => 29,
            Self::Heartbeat => 30,
            Self::Accepted => 40,
            Self::Reply(_) => 41,
            Self::Error(_) => 42,
            Self::ResultBegin => 43,
            Self::ResultEnd { .. } => 44,
            Self::BatchReady(_) => 45,
        })
    }

    /// Priority traffic never queues behind normal worker operations.
    #[must_use]
    pub fn priority(&self) -> bool {
        (20..=30).contains(&self.kind().0)
    }
}

impl MessageKind {
    /// Refuse unknown kinds before decoding a payload.
    /// # Errors
    /// Unknown required kind.
    pub fn validate(self) -> Result<(), ProtocolError> {
        if matches!(self.0, 1..=3 | 10..=17 | 20..=30 | 40..=45 | 100) {
            Ok(())
        } else {
            Err(ProtocolError::UnknownKind)
        }
    }
}
