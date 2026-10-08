// SPDX-License-Identifier: Apache-2.0
//! `table_structure`, `table_sql`, `table_page` and `table_count`.
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use openrows_core::script::{ScriptCanceller, ScriptOutcome, run_bound};
use openrows_core::{CoreError, QueryEvent};
use openrows_results::ResultStore;
use openrows_sql::select::{BoundSql, Page, count_sql, page_sql};
use openrows_sql::{Dialect, StatementKind, classify};
use openrows_types::{QueryId, SessionId};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use super::dto::{
    CountReply, CountRequest, PageReply, PageRequest, SqlPreviewDto, TableQueryDto,
    TableStructureDto, TableTarget,
};
use crate::connections::catalog::{read_columns, read_structure};
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::queries::Queries;
use crate::queries::events::{QueryEventDto, finished, translate};

/// Largest page a table view may request.
const MAX_PAGE: u32 = 5_000;

/// Columns, indexes and row estimate of a relation, read on the metadata session.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_structure(
    connections: State<'_, Connections>,
    target: TableTarget,
) -> Result<TableStructureDto, IpcError> {
    let structure = read_structure(
        &connections,
        (&target.profile_id, &target.database),
        (target.schema, target.table),
    )
    .await?;
    Ok(structure.into())
}

/// The page `SELECT` and its bind parameters, without running it (SQL preview).
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_sql(
    connections: State<'_, Connections>,
    query: TableQueryDto,
    offset: u32,
    limit: u32,
) -> Result<SqlPreviewDto, IpcError> {
    let bound = generate(
        &connections,
        &query,
        Some((limit.clamp(1, MAX_PAGE), offset)),
    )
    .await?;
    Ok(SqlPreviewDto {
        sql: bound.sql,
        params: bound.params,
    })
}

/// Start one page on the target database's session; events stream like a query
/// run and `query_cancel(queryId)` cancels it. Nothing is written to history.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_page(
    app: AppHandle,
    connections: State<'_, Connections>,
    queries: State<'_, Queries>,
    request: PageRequest,
    on_event: Channel<QueryEventDto>,
) -> Result<PageReply, IpcError> {
    let session = owned_session(&connections, &request.session_id, &request.query.target)?;
    let limit = request.limit.clamp(1, MAX_PAGE);
    let bound = generate(&connections, &request.query, Some((limit, request.offset))).await?;
    let run = PageRun {
        query_id: QueryId::new().to_string(),
        session,
        bound,
        cancel: ScriptCanceller::default(),
        events: on_event,
    };
    queries.track(&run.query_id, run.cancel.clone());
    let reply = PageReply {
        query_id: run.query_id.clone(),
        sql: run.bound.sql.clone(),
    };
    tauri::async_runtime::spawn(stream_page(app, run));
    Ok(reply)
}

/// Exact `count(*)` with the current filters; `query_cancel(ticket)` stops it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_count(
    connections: State<'_, Connections>,
    queries: State<'_, Queries>,
    store: State<'_, Arc<ResultStore>>,
    request: CountRequest,
) -> Result<CountReply, IpcError> {
    let session = owned_session(&connections, &request.session_id, &request.query.target)?;
    let bound = generate(&connections, &request.query, None).await?;
    let cancel = ScriptCanceller::default();
    queries.track(&request.ticket, cancel.clone());
    let (mut result, mut failure) = (None, None);
    let outcome = run_bound(
        &connections.sessions,
        session,
        bound,
        &cancel,
        |event| match event.event {
            QueryEvent::ResultBegin { result: id, .. } => result = Some(id.to_string()),
            QueryEvent::Failed { error } => failure = Some(error),
            _ => {}
        },
    )
    .await;
    queries.forget(&request.ticket);
    let rows = result.and_then(|id| {
        let rows = store.first_integer(&id);
        store.remove(&id);
        rows
    });
    count_reply(outcome, rows, failure)
}

/// Everything a started page needs after the command returned.
struct PageRun {
    query_id: String,
    session: SessionId,
    bound: BoundSql,
    cancel: ScriptCanceller,
    events: Channel<QueryEventDto>,
}

async fn stream_page(app: AppHandle, run: PageRun) {
    let connections = app.state::<Connections>();
    let store = app.state::<Arc<ResultStore>>();
    let started = Instant::now();
    let outcome = run_bound(
        &connections.sessions,
        run.session,
        run.bound,
        &run.cancel,
        |event| {
            // A closed channel means the tab is gone; the page still finishes.
            let _ = run.events.send(translate(event, &store));
        },
    )
    .await;
    let _ = run.events.send(finished(outcome, started.elapsed()));
    app.state::<Queries>().forget(&run.query_id);
}

/// Validate identifiers against the catalog columns and generate the page
/// (`Some((limit, offset))`) or count SQL; it must classify as a plain read.
async fn generate(
    connections: &Connections,
    query: &TableQueryDto,
    page: Option<(u32, u32)>,
) -> Result<BoundSql, IpcError> {
    let target = &query.target;
    let columns = read_columns(
        connections,
        (&target.profile_id, &target.database),
        (target.schema.clone(), target.table.clone()),
    )
    .await?;
    if columns.is_empty() {
        return Err(IpcError::new(
            IpcErrorKind::NotFound,
            format!("{}.{} was not found", target.schema, target.table),
        ));
    }
    let known: Vec<String> = columns.into_iter().map(|column| column.name).collect();
    let model = query.to_query();
    let bound = match page {
        Some((limit, offset)) => page_sql(
            Dialect::Postgres,
            &model,
            &known,
            Page {
                limit,
                offset: offset.into(),
            },
        ),
        None => count_sql(Dialect::Postgres, &model, &known),
    }
    .map_err(|error| IpcError::invalid(error.to_string()))?;
    ensure_read(&bound)?;
    Ok(bound)
}

/// Defence in depth: table browsing only ever runs plain reads.
fn ensure_read(bound: &BoundSql) -> Result<(), IpcError> {
    let class = classify(&bound.sql, Dialect::Postgres);
    if class.kind == StatementKind::Read && !class.dangerous {
        Ok(())
    } else {
        Err(IpcError::new(
            IpcErrorKind::Internal,
            "The generated table query is not a plain read",
        ))
    }
}

/// The open session, which must belong to the target's saved connection and
/// database (each opened database has its own session).
pub(crate) fn owned_session(
    connections: &Connections,
    session_id: &str,
    target: &TableTarget,
) -> Result<SessionId, IpcError> {
    let session =
        SessionId::from_str(session_id).map_err(|_| IpcError::invalid("Invalid session id"))?;
    let owner = connections
        .owners
        .owner(session)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "The session is not open"))?;
    if owner.profile.to_string() != target.profile_id {
        return Err(IpcError::invalid(
            "The session belongs to another connection",
        ));
    }
    if owner.database != target.database {
        return Err(IpcError::invalid("The session runs on another database"));
    }
    Ok(session)
}

#[expect(clippy::cast_precision_loss, reason = "row counts stay far below 2^53")]
fn count_reply(
    outcome: ScriptOutcome,
    rows: Option<i64>,
    failure: Option<CoreError>,
) -> Result<CountReply, IpcError> {
    match outcome {
        ScriptOutcome::Cancelled => Ok(CountReply::Cancelled),
        ScriptOutcome::Completed => rows
            .map(|rows| CountReply::Counted { rows: rows as f64 })
            .ok_or_else(|| IpcError::new(IpcErrorKind::Internal, "The count returned no number")),
        ScriptOutcome::Failed => Err(failure.map_or_else(
            || IpcError::new(IpcErrorKind::Internal, "The count failed"),
            IpcError::from,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_core::ErrorKind;

    #[test]
    fn only_plain_reads_pass() {
        let read = BoundSql {
            sql: "SELECT * FROM \"t\" LIMIT 1 OFFSET 0".to_owned(),
            params: Vec::new(),
        };
        assert!(ensure_read(&read).is_ok());
        let write = BoundSql {
            sql: "DELETE FROM \"t\"".to_owned(),
            params: Vec::new(),
        };
        assert!(ensure_read(&write).is_err());
    }

    #[test]
    fn counts_map_outcomes() {
        assert_eq!(
            count_reply(ScriptOutcome::Completed, Some(3), None).ok(),
            Some(CountReply::Counted { rows: 3.0 })
        );
        assert_eq!(
            count_reply(ScriptOutcome::Cancelled, None, None).ok(),
            Some(CountReply::Cancelled)
        );
        let failure = CoreError {
            kind: ErrorKind::Server,
            message: "canceled".to_owned(),
            db_code: Some("57014".to_owned()),
        };
        assert!(count_reply(ScriptOutcome::Failed, None, Some(failure)).is_err());
        assert!(count_reply(ScriptOutcome::Completed, None, None).is_err());
    }
}
