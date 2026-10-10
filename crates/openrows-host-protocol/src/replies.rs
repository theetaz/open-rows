// SPDX-License-Identifier: Apache-2.0
//! Owned replies preserve driver error meaning without serializing native enums.
use openrows_driver_api::{ConnectTrace, DriverError, ExecSummary};
use serde::{Deserialize, Serialize};

/// Facets actually exposed by the host, not the native driver's hidden methods.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent wire facet flags, never lifecycle state"
)]
pub struct Facets {
    /// Catalog implemented.
    pub catalog: bool,
    /// Editor implemented.
    pub editor: bool,
    /// Admin implemented.
    pub admin: bool,
    /// Bulk implemented.
    pub bulk: bool,
}
/// Stage IDs/durations retained without dependence on the driver enum layout.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireTrace {
    /// Stage ID and elapsed nanoseconds.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub completed: Vec<(u8, u64)>,
    /// Failed stage ID.
    pub failed: Option<u8>,
    /// Whether TLS was retried.
    pub retried_tls: bool,
    /// Native file registry recorded local transport; false before it opens.
    pub local_tls: bool,
}
impl From<&ConnectTrace> for WireTrace {
    fn from(trace: &ConnectTrace) -> Self {
        Self {
            completed: trace
                .completed
                .iter()
                .map(|(stage, elapsed)| {
                    (
                        stage_id(*stage),
                        u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
                    )
                })
                .collect(),
            failed: trace.failed.map(stage_id),
            retried_tls: trace.retried_with_tls,
            local_tls: matches!(trace.tls, Some(openrows_driver_api::TlsStatus::LocalSocket)),
        }
    }
}
fn stage_id(stage: openrows_driver_api::ConnectStage) -> u8 {
    use openrows_driver_api::ConnectStage;
    match stage {
        ConnectStage::Resolve => 1,
        ConnectStage::Connect => 2,
        ConnectStage::Tls => 3,
        ConnectStage::Authenticate => 4,
        ConnectStage::ServerInfo => 5,
        ConnectStage::StartupCommands => 6,
    }
}
/// Server identity for the current local-file registry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Connected {
    /// Product.
    pub product: String,
    /// Library version.
    pub version: String,
    /// Optional flavour.
    pub flavour: Option<String>,
    /// Database.
    pub database: String,
    /// User.
    pub user: String,
    /// TLS status ID (2 means local transport).
    pub tls: u8,
    /// Stage trace.
    pub trace: WireTrace,
    /// Actual exposed facets.
    pub facets: Facets,
    /// Current transaction state.
    pub in_transaction: bool,
}
/// Statement completion, including data-lane barrier.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireSummary {
    /// Delivered rows.
    pub rows: u64,
    /// Delivered batches.
    pub batches: u64,
    /// Command tag.
    pub tag: String,
    /// First-batch nanoseconds.
    pub first_batch_ns: Option<u64>,
    /// Transaction state.
    pub in_transaction: bool,
    /// Last result ordinal.
    pub result: u32,
    /// Last data fragment sequence of that result.
    pub sequence: u64,
}
impl From<ExecSummary> for WireSummary {
    fn from(summary: ExecSummary) -> Self {
        Self {
            rows: summary.rows,
            batches: summary.batches,
            tag: summary.command_tag,
            first_batch_ns: summary
                .first_batch_latency
                .map(|value| u64::try_from(value.as_nanos()).unwrap_or(u64::MAX)),
            ..Self::default()
        }
    }
}
/// Terminal success payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Reply {
    /// Connection opened.
    Connected(Connected),
    /// Statement finished after the final data barrier.
    Executed(WireSummary),
    /// Bounded native facet response.
    Facet {
        /// Actual response.
        value: crate::FacetReply,
        /// Post-operation transaction state.
        in_transaction: bool,
    },
    /// Facet stream ended after its final acknowledged fragment.
    FacetChunks {
        /// Final chunk ordinal.
        sequence: u64,
        /// Post-operation transaction state.
        in_transaction: bool,
    },
    /// Worker round trip succeeded.
    Pong,
}
/// Typed driver failures; host integrity failures terminate the transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WireError {
    /// Database error; may contain user-visible database detail, never diagnostics.
    Server {
        /// SQLSTATE.
        code: String,
        /// Database error.
        message: String,
    },
    /// Unsupported operation or explicit cell ceiling.
    Unsupported(String),
    /// Value decode failure.
    Decode {
        /// Column.
        column: u32,
        /// Reason.
        reason: String,
    },
    /// Authentication refused.
    Auth,
    /// TLS failed.
    Tls,
    /// Transport failed.
    Network,
    /// Declared host failure: 1 Spawn, 2 Incompatible, 3 Exited, 4 `MemoryLimit`,
    /// 5 Unresponsive, 6 Protocol; unknown IDs are terminal protocol failures.
    Host(u8),
    /// Host integrity failed.
    Protocol,
}
impl From<DriverError> for WireError {
    fn from(error: DriverError) -> Self {
        match error {
            DriverError::Server { code, message } => Self::Server { code, message },
            DriverError::Unsupported(value) => Self::Unsupported(value),
            DriverError::Decode { column, reason } => Self::Decode {
                column: u32::try_from(column).unwrap_or(u32::MAX),
                reason,
            },
            DriverError::Auth(_) => Self::Auth,
            DriverError::Tls(_) => Self::Tls,
            DriverError::Network(_) => Self::Network,
            DriverError::Host(reason) => Self::Host(match reason {
                openrows_driver_api::HostFailure::Spawn => 1,
                openrows_driver_api::HostFailure::Incompatible => 2,
                openrows_driver_api::HostFailure::Exited => 3,
                openrows_driver_api::HostFailure::MemoryLimit => 4,
                openrows_driver_api::HostFailure::Unresponsive => 5,
                openrows_driver_api::HostFailure::Protocol => 6,
            }),
            DriverError::Protocol(_) | DriverError::Arrow(_) => Self::Protocol,
        }
    }
}
