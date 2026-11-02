// SPDX-License-Identifier: Apache-2.0
//! Bounded metadata from actual renderer requests, without altering Tauri internals.
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

const MAX_RECORDS: usize = 64;
static RECORDS: OnceLock<Mutex<Observations>> = OnceLock::new();

#[derive(Clone, Default, Serialize)]
pub(super) struct Observations {
    query: VecDeque<Value>,
    windows: VecDeque<Value>,
}

pub(super) fn install() {
    let _ = RECORDS.set(Mutex::new(Observations::default()));
}

pub(crate) fn query(event: &crate::queries::events::QueryEventDto) {
    if let crate::queries::events::QueryEventDto::ResultEnd {
        statement,
        result_id,
        rows,
        truncated,
    } = event
    {
        record(
            serde_json::json!({"type":"resultEnd","statement":statement,
            "resultId":result_id,"rows":rows,"truncated":truncated}),
            true,
        );
    }
}

pub(crate) fn window(request: Value) {
    record(request, false);
}

fn record(value: Value, query: bool) {
    if let Some(records) = RECORDS.get()
        && let Ok(mut records) = records.lock()
    {
        append(
            if query {
                &mut records.query
            } else {
                &mut records.windows
            },
            value,
        );
    }
}

fn append(records: &mut VecDeque<Value>, value: Value) {
    if records.len() == MAX_RECORDS {
        records.pop_front();
    }
    records.push_back(value);
}

#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(super) enum ObservationError {
    #[error("Automation is disabled")]
    Disabled,
    #[error("Automation state unavailable")]
    State,
}

#[tauri::command]
pub(super) fn automation_observations() -> Result<Observations, ObservationError> {
    RECORDS
        .get()
        .ok_or(ObservationError::Disabled)?
        .lock()
        .map(|records| records.clone())
        .map_err(|_| ObservationError::State)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observations_keep_only_the_last_bounded_records() {
        let mut records = VecDeque::new();
        for number in 0..100 {
            append(&mut records, serde_json::json!({"rowStart":number}));
        }
        assert_eq!(records.len(), MAX_RECORDS);
        assert_eq!(
            records.front().map(|value| value["rowStart"].clone()),
            Some(Value::from(36))
        );
        assert_eq!(
            records.back().map(|value| value["rowStart"].clone()),
            Some(Value::from(99))
        );
    }
}
