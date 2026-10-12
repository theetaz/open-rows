// SPDX-License-Identifier: Apache-2.0
//! Fixed read-only statements under server read/result/time caps and a client deadline.
use crate::session::ClickHouseSession;
use openrows_driver_api::DriverError;
use serde_json::Value;
use std::time::Duration;

/// Bounded text rows; oversized cells become NULL and mark the reading partial.
#[derive(Debug, Default)]
pub(super) struct Bounded {
    pub(super) rows: Vec<Vec<Option<String>>>,
    pub(super) partial: bool,
}

/// Server guards, applied before the read-only switch in the same statement:
/// a failure to apply any of them fails the read instead of running unbounded.
/// Zero-valued events are listed so a real zero never becomes unknown.
pub(super) fn settings(deadline: u32) -> String {
    let seconds = f64::from(deadline.clamp(1, 1_000)) / 1_000.0;
    format!(
        " SETTINGS max_execution_time = {seconds}, timeout_overflow_mode = 'throw', max_result_rows = 1001, result_overflow_mode = 'throw', max_rows_to_read = 20000000, read_overflow_mode = 'throw', system_events_show_zero_values = 1, readonly = 1"
    )
}

/// One guarded statement; `{p1:String}` binds this session's query-id prefix.
pub(super) async fn query(
    session: &ClickHouseSession,
    sql: &str,
    deadline: u32,
) -> Result<Bounded, DriverError> {
    let sql = format!("{sql}{}", settings(deadline));
    let params = [Some(session.id.clone())];
    let read = session.bounded_query(&sql, &params, 1_048_576);
    let rows = tokio::time::timeout(Duration::from_millis(1_500), read)
        .await
        .map_err(|_| DriverError::Protocol("monitor deadline exceeded".into()))??;
    Ok(bound(rows))
}

pub(super) fn bound(rows: Vec<Vec<Value>>) -> Bounded {
    let mut bounded = Bounded {
        partial: rows.len() > 1_000,
        rows: Vec::with_capacity(rows.len().min(1_000)),
    };
    for row in rows.into_iter().take(1_000) {
        let cells = row
            .into_iter()
            .map(|value| {
                let text = match value {
                    Value::Null => return None,
                    Value::String(text) => text,
                    other => other.to_string(),
                };
                bounded.partial |= text.len() >= 4_096;
                (text.len() < 4_096).then_some(text)
            })
            .collect();
        bounded.rows.push(cells);
    }
    bounded
}
