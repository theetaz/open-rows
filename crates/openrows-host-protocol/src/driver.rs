// SPDX-License-Identifier: Apache-2.0
//! Closed, secret-free launch identities; production engines register here at M2-04.

/// Compile-time launch target, independent of profile fields or arbitrary argv.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HostDriverId {
    /// Handshake/shutdown diagnostics only; exposes no database capabilities.
    Probe,
    /// Optional native fixture; production helpers reject this target.
    SqliteTest,
}
impl HostDriverId {
    /// The single non-secret command argument.
    #[must_use]
    pub const fn argument(self) -> &'static str {
        match self {
            Self::Probe => "probe",
            Self::SqliteTest => "sqlite",
        }
    }
    /// Parse only known launch identities; no profiles, paths or commands are accepted.
    #[must_use]
    pub fn parse(argument: &str) -> Option<Self> {
        match argument {
            "probe" => Some(Self::Probe),
            "sqlite" => Some(Self::SqliteTest),
            _ => None,
        }
    }
}
