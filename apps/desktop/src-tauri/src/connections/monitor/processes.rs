// SPDX-License-Identifier: Apache-2.0
//! Processes and Monitor subscribers share the same activity read.
use crate::connections::{Connections, IpcError, processes_dto::ProcessDto};
use openrows_driver_api::ProcessInfo;

/// Reuse the permitted current monitor sample without opening another session.
pub(crate) fn shared(
    state: &Connections,
    scope: (&str, &str),
    application: &str,
) -> Option<Result<Vec<ProcessDto>, IpcError>> {
    let key = (scope.0.into(), scope.1.into());
    if !state.monitors.contains(&key) {
        return None;
    }
    let result = state
        .monitors
        .get(&key)
        .and_then(|monitor| monitor.activity());
    Some(result.map_err(|_| IpcError::invalid("The shared monitor activity sample is pending, restricted or stale; inspect monitor capabilities"))
        .and_then(|reading| reading.rows.iter().map(|row| convert(row, application)).collect()))
}

fn convert(row: &[Option<String>], application: &str) -> Result<ProcessDto, IpcError> {
    let text = |index: usize| row.get(index).cloned().flatten();
    let id = text(0)
        .and_then(|id| id.parse().ok())
        .ok_or_else(|| IpcError::invalid("Invalid monitored process identity"))?;
    let process = ProcessInfo {
        id,
        state: text(1),
        wait: text(2),
        duration_ms: text(3).and_then(|ms| ms.parse().ok()),
        query: text(6),
        user: text(7),
        database: text(8),
        application: text(9),
        query_start: text(10),
        ..ProcessInfo::default()
    };
    ProcessDto::new(process, application)
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_activity_uses_redacted_columns_and_never_executes_sql()
    -> Result<(), crate::connections::IpcError> {
        let row = [
            Some("7".into()),
            Some("active".into()),
            None,
            Some("25".into()),
            None,
            None,
            Some("SELECT ?".into()),
        ];
        let value = serde_json::to_value(super::convert(&row, "OpenRows")?)
            .map_err(|_| crate::connections::IpcError::invalid("DTO"))?;
        assert_eq!(value["query"], "SELECT ?");
        assert_eq!(value["id"], 7);
        Ok(())
    }
}
