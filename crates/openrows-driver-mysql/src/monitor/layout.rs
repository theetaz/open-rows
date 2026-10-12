// SPDX-License-Identifier: Apache-2.0
//! Distinct engine/version contracts; source probes decide actual availability.
use openrows_monitor::{Column, MetricKind, SampleState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Layout {
    pub(super) maria: bool,
    pub(super) modern_process: bool,
    pub(super) modern_locks: bool,
}
impl Layout {
    pub(super) fn detect(version: &str) -> Option<Self> {
        let maria = version.contains("MariaDB");
        let version = version.strip_prefix("5.5.5-").unwrap_or(version);
        let mut parts = version.split(['.', '-']);
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next()?.parse::<u32>().ok()?;
        let patch = parts.next()?.parse::<u32>().ok()?;
        let number = (major, minor, patch);
        let supported = if maria {
            number >= (10, 6, 0) && major <= 11
        } else {
            (major == 5 && minor == 7) || major == 8 || major == 9
        };
        supported.then_some(Self {
            maria,
            modern_process: !maria && number >= (8, 0, 22),
            modern_locks: !maria && major >= 8,
        })
    }
    pub(super) fn status(self) -> &'static str {
        if self.maria {
            "information_schema.GLOBAL_STATUS"
        } else {
            "performance_schema.global_status"
        }
    }
    pub(super) fn replication_grant(self) -> &'static str {
        if self.maria {
            "SLAVE MONITOR (REPLICA MONITOR alias)"
        } else {
            "REPLICATION CLIENT"
        }
    }
}
pub(super) fn unavailable(reason: &str) -> SampleState {
    SampleState::Unavailable(reason.into())
}
pub(super) fn columns(spec: &str) -> Vec<Column> {
    spec.split(',')
        .filter_map(|part| part.split_once(':'))
        .map(|(name, unit)| Column {
            name: name.into(),
            unit: match unit {
                "counter" if name.ends_with("ms") => "ms",
                "counter" if name.ends_with("bytes") => "bytes",
                "counter" => "count",
                other => other,
            }
            .into(),
            kind: match unit {
                "counter" => MetricKind::Counter,
                "text" => MetricKind::Text,
                "query" => MetricKind::Query,
                _ => MetricKind::Gauge,
            },
        })
        .collect()
}
