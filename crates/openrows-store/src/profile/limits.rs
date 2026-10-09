// SPDX-License-Identifier: Apache-2.0
//! Per-connection session limits (M1-01, docs/04 §3.2 and §6): how long one
//! statement may run and how long the connection may sit idle before its
//! sessions are closed (they reopen on next use).

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Session limits of a profile; absent from files that keep the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLimits {
    /// Seconds a statement may run before it is cancelled; `0` means no
    /// limit and `None` follows the app-wide default in the settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement_timeout_seconds: Option<u32>,
    /// Minutes without a statement after which the connection's sessions
    /// are closed; `0` (the default) never closes them.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub idle_timeout_minutes: u32,
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes the field by reference"
)]
fn is_zero(minutes: &u32) -> bool {
    *minutes == 0
}

impl SessionLimits {
    /// True when the profile keeps every default (nothing is written).
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The statement timeout to apply, `default_seconds` when the profile
    /// follows the app-wide default; `None` means no limit.
    #[must_use]
    pub fn statement_timeout(&self, default_seconds: u32) -> Option<Duration> {
        let seconds = self.statement_timeout_seconds.unwrap_or(default_seconds);
        (seconds > 0).then(|| Duration::from_secs(u64::from(seconds)))
    }

    /// Idle time after which the sessions close; `None` never closes them.
    #[must_use]
    pub fn idle_timeout(&self) -> Option<Duration> {
        (self.idle_timeout_minutes > 0)
            .then(|| Duration::from_secs(u64::from(self.idle_timeout_minutes) * 60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_values_override_the_default_and_zero_means_none() {
        let follows = SessionLimits::default();
        assert_eq!(follows.statement_timeout(0), None);
        assert_eq!(follows.statement_timeout(30), Some(Duration::from_secs(30)));
        assert_eq!(follows.idle_timeout(), None);
        let own = SessionLimits {
            statement_timeout_seconds: Some(0),
            idle_timeout_minutes: 15,
        };
        assert_eq!(own.statement_timeout(30), None);
        assert_eq!(own.idle_timeout(), Some(Duration::from_secs(900)));
    }

    #[test]
    fn defaults_are_not_written() -> Result<(), serde_json::Error> {
        assert_eq!(serde_json::to_string(&SessionLimits::default())?, "{}");
        let limits = SessionLimits {
            statement_timeout_seconds: Some(5),
            idle_timeout_minutes: 10,
        };
        let text = serde_json::to_string(&limits)?;
        assert_eq!(
            text,
            r#"{"statementTimeoutSeconds":5,"idleTimeoutMinutes":10}"#
        );
        assert_eq!(serde_json::from_str::<SessionLimits>(&text)?, limits);
        Ok(())
    }
}
