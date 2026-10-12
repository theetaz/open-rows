// SPDX-License-Identifier: Apache-2.0
//! Keep named replica state/lag only; never retain addresses or server error text.
use super::{bounded::Bounded, layout::columns};
use openrows_monitor::{Reading, SampleState};
pub(super) const FIELDS: &[&str] = &[
    "Channel_Name",
    "Connection_name",
    "Replica_IO_Running",
    "Slave_IO_Running",
    "Replica_SQL_Running",
    "Slave_SQL_Running",
    "Seconds_Behind_Source",
    "Seconds_Behind_Master",
];
pub(super) fn reading(result: &Bounded) -> Reading {
    let value = |row: &[Option<String>], alternatives: &[&str]| {
        result
            .names
            .iter()
            .position(|n| alternatives.iter().any(|a| n.eq_ignore_ascii_case(a)))
            .and_then(|i| row.get(i))
            .cloned()
            .flatten()
    };
    let rows = result
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let io = value(row, &["Replica_IO_Running", "Slave_IO_Running"]);
            let sql = value(row, &["Replica_SQL_Running", "Slave_SQL_Running"]);
            let lag = if io.as_deref() == Some("Yes") && sql.as_deref() == Some("Yes") {
                value(row, &["Seconds_Behind_Source", "Seconds_Behind_Master"])
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .map(|v| (v * 1_000.0).to_string())
            } else {
                None
            };
            vec![
                value(row, &["Channel_Name", "Connection_name"])
                    .or_else(|| Some(index.to_string())),
                io,
                sql,
                lag,
            ]
        })
        .collect::<Vec<_>>();
    let state = if rows.is_empty() {
        SampleState::Unavailable("Not applicable: no replica channels".into())
    } else {
        SampleState::Ok
    };
    Reading {
        terminal: false,
        state,
        epoch: "best effort".into(),
        columns: columns("channel_id:text,io_state:text,sql_state:text,replay_lag_ms:ms"),
        partial: result.partial || rows.len() >= 1000,
        rows,
    }
}
