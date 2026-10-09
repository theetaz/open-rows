// SPDX-License-Identifier: Apache-2.0
//! The latency step and the TLS summary line of Test connection.
use std::time::Duration;

use openrows_net::tls::{TlsDetails, Verification};

use super::super::super::error::IpcError;
use super::super::probe::LatencyDto;
use super::{StepStatus, Steps, TestStepKind};

impl Steps {
    /// The latency step: the median round trip of `SELECT 1`, or a warning
    /// when the probe failed (the connection itself worked).
    pub(crate) fn latency(&mut self, result: &Result<LatencyDto, IpcError>) {
        let label = "Round trip (SELECT 1)".to_owned();
        match result {
            Ok(latency) => {
                let median = Duration::from_secs_f64(latency.median_ms / 1e3);
                let step = self.timed(TestStepKind::Latency, label, median);
                step.detail = Some(format!(
                    "median {:.1} ms · min {:.1} ms · max {:.1} ms over {} queries",
                    latency.median_ms, latency.min_ms, latency.max_ms, latency.samples
                ));
            }
            Err(error) => {
                let step = self.push(TestStepKind::Latency, label, StepStatus::Warning);
                step.detail = Some(error.message.clone());
            }
        }
    }
}

/// `TLS 1.3 · cipher · subject · verification` for the TLS step.
pub(super) fn describe(details: &TlsDetails) -> String {
    let check = match details.verification {
        Verification::Full => "certificate verified",
        Verification::Chain => "CA verified, host name not checked",
        Verification::None => "certificate NOT verified",
    };
    let mut parts = vec![details.protocol.clone(), details.cipher.clone()];
    parts.extend(details.subject.clone());
    parts.push(check.to_owned());
    parts.retain(|part| !part.is_empty());
    parts.join(" · ")
}
