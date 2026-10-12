// SPDX-License-Identifier: Apache-2.0
//! Fixed read-only batches with a lock timeout, an attention deadline and
//! row/byte admission before text allocation.
use std::sync::Arc;
use std::time::Duration;

use openrows_driver_api::{DriverError, Session};

use crate::decode::{Kind, kind, text};
use crate::reply::Rows;
use crate::session::SqlServerSession;
use crate::tds::token::{Column, RowValues};

const MAX_ROWS: usize = 1_000;
const MAX_BYTES: usize = 1_024 * 1_024;
const MAX_CELL: usize = 4_096;

/// Text rows admitted under the monitor caps.
#[derive(Default)]
pub(super) struct Bounded {
    kinds: Vec<Kind>,
    pub(super) rows: Vec<Vec<Option<String>>>,
    pub(super) partial: bool,
    bytes: usize,
}

impl Rows for Bounded {
    fn begin(&mut self, columns: &Arc<[Column]>) -> Result<(), DriverError> {
        if columns.len() > 128 {
            return Err(limit());
        }
        self.kinds = columns.iter().map(|c| kind(&c.info.sql)).collect();
        Ok(())
    }

    fn row(&mut self, values: RowValues<'_>) -> Result<bool, DriverError> {
        self.bytes += values.iter().flatten().map(|v| v.len()).sum::<usize>();
        if self.rows.len() >= MAX_ROWS || self.bytes > MAX_BYTES {
            return Err(limit());
        }
        let mut row = Vec::with_capacity(values.len());
        for (column, (value, kind)) in values.iter().zip(&self.kinds).enumerate() {
            let cell = match value.as_deref() {
                Some(bytes) if bytes.len() >= MAX_CELL => {
                    self.partial = true;
                    None
                }
                Some(bytes) => {
                    Some(
                        text::render(*kind, bytes).map_err(|reason| DriverError::Decode {
                            column,
                            reason: reason.to_string(),
                        })?,
                    )
                }
                None => None,
            };
            row.push(cell);
        }
        self.rows.push(row);
        Ok(true)
    }

    fn end(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}

fn limit() -> DriverError {
    DriverError::Protocol("monitor result limit exceeded".into())
}

/// Run `sql` after `SET LOCK_TIMEOUT 100` (ADR-0011; SQL Server has no
/// statement timeout); at three quarters of the deadline the
/// attention signal cancels it, keeping the session usable when the server
/// acknowledges in time. A missing acknowledgement loses the session.
pub(super) async fn query(
    session: &mut SqlServerSession,
    sql: &str,
    deadline: u32,
) -> Result<Bounded, DriverError> {
    if session.monitor.lost {
        return Err(DriverError::Protocol("monitor session lost".into()));
    }
    let deadline = u64::from(deadline.clamp(4, 1_000));
    let batch = format!("SET LOCK_TIMEOUT 100; {sql}");
    let cancel = session.cancel_handle();
    let mut rows = Bounded::default();
    let outcome = {
        let mut request = std::pin::pin!(session.request(&batch, &[], &mut rows));
        let first = Duration::from_millis(deadline * 3 / 4);
        if let Ok(result) = tokio::time::timeout(first, &mut request).await {
            Some(result)
        } else {
            if let Some(cancel) = cancel {
                let _ = cancel.cancel().await;
            }
            let rest = Duration::from_millis(deadline / 4);
            tokio::time::timeout(rest, &mut request).await.ok()
        }
    };
    match outcome {
        Some(Ok(_)) => Ok(rows),
        Some(Err(error)) => {
            if !matches!(error, DriverError::Server { .. } | DriverError::Auth(_)) {
                session.monitor.lost = true;
            }
            Err(error)
        }
        None => {
            session.monitor.lost = true;
            Err(DriverError::Protocol("monitor deadline exceeded".into()))
        }
    }
}
