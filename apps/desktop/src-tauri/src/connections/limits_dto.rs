// SPDX-License-Identifier: Apache-2.0
//! Session limits as the connection form shows and sends them (M1-01).
use openrows_store::profile::SessionLimits;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Statement and idle timeouts of a connection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LimitsDto {
    /// Seconds a statement may run; `0` is no limit, `null` follows the
    /// app-wide default (Settings ▸ General).
    statement_timeout_seconds: Option<u32>,
    /// Minutes without a query before the connection's sessions close
    /// (reopened on next use); `0` never closes them.
    idle_timeout_minutes: u32,
}

impl From<SessionLimits> for LimitsDto {
    fn from(limits: SessionLimits) -> Self {
        Self {
            statement_timeout_seconds: limits.statement_timeout_seconds,
            idle_timeout_minutes: limits.idle_timeout_minutes,
        }
    }
}

impl From<LimitsDto> for SessionLimits {
    fn from(limits: LimitsDto) -> Self {
        Self {
            statement_timeout_seconds: limits.statement_timeout_seconds,
            idle_timeout_minutes: limits.idle_timeout_minutes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_round_trip_and_default_when_absent() -> Result<(), serde_json::Error> {
        let dto: LimitsDto =
            serde_json::from_str(r#"{"statementTimeoutSeconds":null,"idleTimeoutMinutes":15}"#)?;
        let limits = SessionLimits::from(dto);
        assert_eq!(limits.idle_timeout_minutes, 15);
        assert_eq!(limits.statement_timeout_seconds, None);
        assert_eq!(LimitsDto::from(limits), dto);
        Ok(())
    }
}
