// SPDX-License-Identifier: Apache-2.0
//! Explicit connection safety and lossless legacy profile decoding.
use serde::{Deserialize, Deserializer, Serialize};

/// Per-connection execution policy (FR-APP-11).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Safety {
    /// Run ordinary statements without approval.
    #[default]
    Unrestricted,
    /// Confirm every statement.
    ConfirmAll,
    /// Confirm statements that may modify data.
    ConfirmWrites,
    /// Authenticate every statement.
    AuthenticateAll,
    /// Authenticate statements that may modify data.
    AuthenticateWrites,
    /// Refuse statements that may modify data.
    ReadOnly,
}

impl Safety {
    /// Whether statements are refused on a read-only connection.
    #[must_use]
    pub fn read_only(self) -> bool {
        self == Self::ReadOnly
    }

    /// Whether the policy requires approval of writes.
    #[must_use]
    pub fn confirm_writes(self) -> bool {
        !matches!(self, Self::Unrestricted | Self::ReadOnly)
    }

    /// Whether execution requires native authentication.
    #[must_use]
    pub fn authenticates(self) -> bool {
        matches!(self, Self::AuthenticateAll | Self::AuthenticateWrites)
    }

    /// Safe fallback when the platform cannot authenticate.
    #[must_use]
    pub fn fallback(self) -> Self {
        match self {
            Self::AuthenticateAll => Self::ConfirmAll,
            Self::AuthenticateWrites => Self::ConfirmWrites,
            Self::Unrestricted | Self::ConfirmAll | Self::ConfirmWrites | Self::ReadOnly => self,
        }
    }
}

impl<'de> Deserialize<'de> for Safety {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        enum Policy {
            Unrestricted,
            ConfirmAll,
            ConfirmWrites,
            AuthenticateAll,
            AuthenticateWrites,
            ReadOnly,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Stored {
            Policy(Policy),
            Legacy {
                #[serde(rename = "readOnly")]
                read_only: bool,
                #[serde(rename = "confirmWrites")]
                confirm_writes: bool,
            },
        }
        Ok(match Stored::deserialize(deserializer)? {
            Stored::Legacy {
                read_only: true, ..
            }
            | Stored::Policy(Policy::ReadOnly) => Self::ReadOnly,
            Stored::Legacy {
                confirm_writes: true,
                ..
            }
            | Stored::Policy(Policy::ConfirmWrites) => Self::ConfirmWrites,
            Stored::Legacy { .. } | Stored::Policy(Policy::Unrestricted) => Self::Unrestricted,
            Stored::Policy(Policy::ConfirmAll) => Self::ConfirmAll,
            Stored::Policy(Policy::AuthenticateAll) => Self::AuthenticateAll,
            Stored::Policy(Policy::AuthenticateWrites) => Self::AuthenticateWrites,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_booleans_migrate_losslessly() -> Result<(), serde_json::Error> {
        for (read, confirm, expected) in [
            (false, false, Safety::Unrestricted),
            (false, true, Safety::ConfirmWrites),
            (true, false, Safety::ReadOnly),
            (true, true, Safety::ReadOnly),
        ] {
            let text = format!(r#"{{"readOnly":{read},"confirmWrites":{confirm}}}"#);
            assert_eq!(serde_json::from_str::<Safety>(&text)?, expected);
            assert_eq!(
                serde_json::from_str::<Safety>(&serde_json::to_string(&expected)?)?,
                expected
            );
        }
        Ok(())
    }
}
