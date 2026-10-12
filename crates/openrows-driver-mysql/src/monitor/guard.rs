// SPDX-License-Identifier: Apache-2.0
//! Fixed read-only statements with server deadlines and bounded transport admission.
use super::{bounded::Bounded, layout::Layout};
use crate::MySqlSession;
use openrows_driver_api::DriverError;
use std::time::Duration;

pub(super) async fn query(
    session: &mut MySqlSession,
    sql: &str,
    names: Option<&'static [&'static str]>,
) -> Result<Bounded, DriverError> {
    session.wire.packets.limit(1_024 * 1_024);
    let mut rows = Bounded::new(names);
    let result =
        tokio::time::timeout(Duration::from_secs(1), session.wire.query(sql, &mut rows)).await;
    match result {
        Ok(Ok(_)) => Ok(rows),
        Ok(Err(error)) => {
            if !matches!(error, DriverError::Server { .. } | DriverError::Auth(_)) {
                session.wire.packets.invalidate();
            }
            Err(error)
        }
        Err(_) => {
            session.wire.packets.invalidate();
            Err(DriverError::Protocol("monitor deadline exceeded".into()))
        }
    }
}
pub(super) async fn guarded(
    session: &mut MySqlSession,
    layout: Layout,
    sql: &str,
    deadline: u32,
) -> Result<Bounded, DriverError> {
    let deadline = deadline.clamp(1, 1_000);
    let setting = if layout.maria {
        format!(
            "SET SESSION max_statement_time = {}",
            f64::from(deadline) / 1_000.0
        )
    } else {
        format!("SET SESSION max_execution_time = {deadline}")
    };
    query(session, &setting, None).await?;
    query(session, "START TRANSACTION READ ONLY", None).await?;
    let names = (sql.starts_with("SHOW REPLICA") || sql.starts_with("SHOW SLAVE"))
        .then_some(super::replication::FIELDS);
    let result = query(session, sql, names).await;
    if result
        .as_ref()
        .err()
        .is_some_and(|e| !matches!(e, DriverError::Server { .. } | DriverError::Auth(_)))
    {
        return result;
    }
    let end = if result.is_ok() { "COMMIT" } else { "ROLLBACK" };
    if let Err(error) = query(session, end, None).await {
        session.wire.packets.invalidate();
        return Err(error);
    }
    result
}
