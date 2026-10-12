// SPDX-License-Identifier: Apache-2.0
//! One widget run: approved board only, typed binding, a disposable
//! read-only connection and row/time/byte bounds.
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{CancelHandle, Connection, DiscardSink, ResultSink, SinkFlow};
use openrows_monitor::boards::{
    BoardResult, Collector, ParamValue, Widget, bind_widget, parse_board,
};
use openrows_sql::select::BoundSql;
use openrows_store::LocalStore;
use openrows_store::profile::ProfileId;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{Manager, State};

use super::protection::{Protection, ReadOnlyMode, protection};
use super::{board_error, stored};
use crate::connections::drivers::driver;
use crate::connections::monitor::{MonitorTarget, reader};
use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::security::Security;

/// Run one widget of a stored board with the entered parameter values.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoardRun {
    /// Profile and database to run on.
    target: MonitorTarget,
    board_id: String,
    widget_id: String,
    values: Vec<ParamValue>,
    /// An interval refresh rather than an explicit user action.
    automatic: bool,
}

/// A bounded widget result and how it was kept read-only.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BoardRunReply {
    result: BoardResult,
    read_only: ReadOnlyMode,
    elapsed_ms: f64,
}

/// Run a widget; imported boards refuse until approved, and interval
/// refreshes refuse on engines without enforced read-only execution.
#[tauri::command]
#[specta::specta]
pub(crate) async fn board_run(
    state: State<'_, Connections>,
    request: BoardRun,
) -> Result<BoardRunReply, IpcError> {
    let security = state.app.state::<Security>();
    security.admit("board_run")?;
    let (widget, bound, plan) = admit(&state, &request)?;
    let profile = state
        .profiles
        .get(ProfileId::from_str(&request.target.profile_id)?)?;
    crate::security::authorize(&profile, vec![bound.sql.clone()], &request.target.database).await?;
    let connection = reader::connect(&state, &request.target, "board").await?;
    security.admit("board_run")?;
    let started = Instant::now();
    let deadline = Duration::from_millis(u64::from(widget.limits.timeout_ms));
    let result = tokio::time::timeout(deadline, execute(connection, &plan, &bound, &widget))
        .await
        .map_err(|_| {
            IpcError::new(
                IpcErrorKind::Timeout,
                "The widget exceeded its time limit and was cancelled",
            )
        })??;
    Ok(BoardRunReply {
        result,
        read_only: plan.mode,
        elapsed_ms: started.elapsed().as_secs_f64() * 1_000.0,
    })
}

/// The approved widget, its bound statement and the engine protection.
fn admit(
    state: &Connections,
    request: &BoardRun,
) -> Result<(Widget, BoundSql, Protection), IpcError> {
    let profile_id = ProfileId::from_str(&request.target.profile_id)?;
    let local = state.app.state::<LocalStore>();
    let record = stored(&local, &profile_id.to_string(), &request.board_id)?;
    if !record.reviewed {
        return Err(IpcError::invalid(
            "Review this imported board's queries and approve it before running them",
        ));
    }
    let board = parse_board(&record.definition).map_err(|error| board_error(&error))?;
    let widget = board
        .widgets
        .iter()
        .find(|widget| widget.id == request.widget_id)
        .cloned()
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "The widget no longer exists"))?;
    let profile = state.profiles.get(profile_id)?;
    let plan = protection(profile.driver, widget.limits.timeout_ms)?;
    if request.automatic && plan.mode != ReadOnlyMode::Enforced {
        return Err(IpcError::invalid(
            "Interval refresh needs engine-enforced read-only execution; refresh manually",
        ));
    }
    let dialect = driver(profile.driver).dialect();
    let bound = bind_widget(&board, &widget, dialect, &request.values)
        .map_err(|error| board_error(&error))?;
    Ok((widget, bound, plan))
}

/// Protect, run and always roll back and close the disposable connection.
async fn execute(
    mut connection: Box<dyn Connection>,
    plan: &Protection,
    bound: &BoundSql,
    widget: &Widget,
) -> Result<BoardResult, IpcError> {
    let mut guard = Interrupt(connection.cancel_handle());
    let result = collect(connection.as_mut(), plan, bound, widget).await;
    if plan.rollback {
        let _ = connection.execute("ROLLBACK", &mut DiscardSink).await;
    }
    guard.0 = None;
    let _ = connection.close().await;
    result
}

async fn collect(
    connection: &mut dyn Connection,
    plan: &Protection,
    bound: &BoundSql,
    widget: &Widget,
) -> Result<BoardResult, IpcError> {
    for setup in &plan.setup {
        connection
            .execute(setup, &mut DiscardSink)
            .await
            .map_err(|error| IpcError::from(&error))?;
    }
    let mut sink = Sink(Collector::new(widget.limits));
    connection
        .execute_bound(&bound.sql, &bound.params, &mut sink)
        .await
        .map_err(|error| IpcError::from(&error))?;
    Ok(sink.0.finish())
}

/// Adapts the bounded collector to the driver's result stream.
struct Sink(Collector);

impl ResultSink for Sink {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.0.begin(&schema);
    }

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if self.0.push(&batch) {
            SinkFlow::Continue
        } else {
            SinkFlow::Stop
        }
    }
}

/// Cancels the running statement when a deadline drops the run.
struct Interrupt(Option<Arc<dyn CancelHandle>>);

impl Drop for Interrupt {
    fn drop(&mut self) {
        if let Some(cancel) = self.0.take() {
            tokio::spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(1), cancel.cancel()).await;
            });
        }
    }
}
