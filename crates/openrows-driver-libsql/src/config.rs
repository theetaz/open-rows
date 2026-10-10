// SPDX-License-Identifier: Apache-2.0
//! Explicit response and batch bounds for Hrana's buffered server replies.
use openrows_driver_api::DriverError;
use std::time::Duration;

/// Hard bounds applied to each HTTP response and each statement's rows.
#[derive(Clone, Copy, Debug)]
pub struct ResultLimits {
    /// Rows delivered per statement; excess rows are drained with a warning.
    pub rows: usize,
    /// Total response bytes, including drained rows; exceeding it fails safely.
    pub response_bytes: usize,
    /// Deadline for one complete pipeline, including body decoding.
    pub timeout: Duration,
}

impl Default for ResultLimits {
    fn default() -> Self {
        Self {
            rows: 10_000,
            response_bytes: 64 * 1024 * 1024,
            timeout: Duration::from_secs(30),
        }
    }
}

impl ResultLimits {
    pub(crate) fn validate(self) -> Result<Self, DriverError> {
        if self.rows == 0
            || self.rows > 1_000_000
            || self.response_bytes < 1024
            || self.response_bytes > 256 * 1024 * 1024
            || self.timeout.is_zero()
            || self.timeout > Duration::from_secs(120)
        {
            return Err(DriverError::Unsupported(
                "invalid libSQL result limits".into(),
            ));
        }
        Ok(self)
    }
}
