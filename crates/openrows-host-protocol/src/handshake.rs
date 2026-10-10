// SPDX-License-Identifier: Apache-2.0
//! One-use authentication precedes version agreement and every credential read.
use crate::{
    BATCH_LIMIT, BUILD, CONTROL_LIMIT, FRAGMENT_LIMIT, LARGE_LIMIT, Lane, PROTOCOL, ProtocolError,
    SCHEMA, WireSecret,
};
use serde::{Deserialize, Serialize};

/// Bounded inherited stdin bootstrap; never contains a profile or SQL.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Bootstrap {
    /// Control endpoint.
    pub control: String,
    /// Data endpoint.
    pub data: String,
    /// Private directory removed by either surviving process.
    pub directory: String,
    /// One-use per-launch secret.
    pub token: WireSecret,
    /// Parent lifetime generation.
    pub parent_generation: u64,
    /// Host generation.
    pub generation: u64,
}
/// First message on each lane; replaying a lane authentication is refused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LaneHello {
    /// One-use token.
    pub token: WireSecret,
    /// Lane identity.
    pub lane: Lane,
    /// Parent lifetime identity.
    pub parent_generation: u64,
    /// Child lifetime identity.
    pub generation: u64,
}
/// Stable numeric feature IDs, separate from native driver enum layouts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireCapabilities {
    /// 1 file, 2 server.
    pub location: u8,
    /// Explicit supported wire feature IDs.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub features: Vec<u16>,
    /// Explicit object kind IDs; empty when catalog is not claimed.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub objects: Vec<u16>,
}
impl WireCapabilities {
    /// Production `DuckDB`: cancellation, text binding and startup SQL only.
    #[must_use]
    pub fn duckdb() -> Self {
        Self {
            location: 1,
            features: vec![1, 2, 11],
            objects: vec![],
        }
    }
    /// Current `SQLite` test-host contract: cancel, bind, startup; implemented catalog/editor facets.
    #[must_use]
    pub fn sqlite() -> Self {
        Self {
            location: 1,
            features: vec![1, 2, 4, 5, 6, 7, 8, 11, 12, 15, 16, 18, 23, 24, 26, 29],
            objects: vec![1, 2],
        }
    }
}
/// Exact lockstep contract sent in Hello and Ready before Connect.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    /// Protocol major/minor.
    pub version: (u16, u16),
    /// Positional layout fingerprint.
    pub schema: String,
    /// Source/dependency build identity.
    pub build: String,
    /// Compile-time registry driver kind.
    pub driver: String,
    /// Child lifetime identity.
    pub generation: u64,
    /// Control, fragment, normal batch, exclusive large limits.
    pub limits: [u32; 4],
    /// Arrow metadata compatibility (V5) and no compression.
    pub arrow: u16,
    /// Required cancel/credits/pause-resume/heartbeat bits.
    pub required: u16,
    /// Supported exposed features.
    pub capabilities: WireCapabilities,
}
impl Hello {
    /// Current exact contract for one generation.
    #[must_use]
    pub fn sqlite(generation: u64) -> Self {
        Self::for_driver(crate::HostDriverId::SqliteTest, generation)
    }
    /// Exact contract for the selected launch target; Probe advertises no engine.
    #[must_use]
    pub fn for_driver(driver: crate::HostDriverId, generation: u64) -> Self {
        Self {
            version: PROTOCOL,
            schema: SCHEMA.into(),
            build: BUILD.into(),
            driver: driver.argument().into(),
            generation,
            limits: [
                u32::try_from(CONTROL_LIMIT).unwrap_or(u32::MAX),
                u32::try_from(FRAGMENT_LIMIT).unwrap_or(u32::MAX),
                u32::try_from(BATCH_LIMIT).unwrap_or(u32::MAX),
                u32::try_from(LARGE_LIMIT).unwrap_or(u32::MAX),
            ],
            arrow: 5,
            required: 15,
            capabilities: match driver {
                crate::HostDriverId::Duckdb => WireCapabilities::duckdb(),
                crate::HostDriverId::SqliteTest => WireCapabilities::sqlite(),
                crate::HostDriverId::Probe => WireCapabilities {
                    location: 1,
                    features: vec![],
                    objects: vec![],
                },
            },
        }
    }
    /// Compare before decoding credentials or opening a database.
    /// # Errors
    /// Any protocol, build, capability or limit disagreement.
    pub fn validate(&self, expected: &Self) -> Result<(), ProtocolError> {
        if self == expected {
            Ok(())
        } else {
            Err(ProtocolError::Incompatible)
        }
    }
}
/// Per-launch authentication state; each lane is accepted exactly once.
pub struct Authentication {
    bootstrap: Bootstrap,
    lanes: [bool; 2],
}
impl Authentication {
    /// Own and eventually zeroize the bootstrap token.
    #[must_use]
    pub fn new(bootstrap: Bootstrap) -> Self {
        Self {
            bootstrap,
            lanes: [false; 2],
        }
    }
    /// Authenticate one lane using a constant-work token comparison.
    /// # Errors
    /// Wrong token, identity, lane or replay.
    pub fn accept(&mut self, hello: &LaneHello, lane: Lane) -> Result<(), ProtocolError> {
        let index = usize::from(lane == Lane::Data);
        let left = hello.token.expose().as_bytes();
        let right = self.bootstrap.token.expose().as_bytes();
        let difference = left
            .iter()
            .zip(right)
            .fold(0, |value, (left, right)| value | (left ^ right));
        if left.is_empty()
            || left.len() != right.len()
            || difference != 0
            || self.lanes[index]
            || hello.lane != lane
            || hello.generation != self.bootstrap.generation
            || hello.parent_generation != self.bootstrap.parent_generation
        {
            return Err(ProtocolError::Authentication);
        }
        self.lanes[index] = true;
        Ok(())
    }
}

/// Serialize a bounded inherited bootstrap, wiping the temporary payload on errors.
/// # Errors
/// Bootstrap exceeds 8 KiB or is invalid.
pub fn encode_bootstrap(bootstrap: &Bootstrap) -> Result<Vec<u8>, ProtocolError> {
    let mut bytes = zeroize::Zeroizing::new(vec![0; 8192]);
    let length = postcard::to_slice(bootstrap, &mut bytes)
        .map_err(|_| ProtocolError::Limit)?
        .len();
    Ok(bytes[..length].to_vec())
}
/// Decode a bounded bootstrap before opening endpoints or reading credentials.
/// # Errors
/// Malformed, empty or oversized bootstrap/token.
pub fn decode_bootstrap(bytes: &[u8]) -> Result<Bootstrap, ProtocolError> {
    if bytes.is_empty() || bytes.len() > 8192 {
        return Err(ProtocolError::Limit);
    }
    let (bootstrap, rest): (Bootstrap, _) =
        postcard::take_from_bytes(bytes).map_err(|_| ProtocolError::Invalid)?;
    if !rest.is_empty() || bootstrap.token.expose().is_empty() {
        return Err(ProtocolError::Authentication);
    }
    Ok(bootstrap)
}
