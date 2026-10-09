// SPDX-License-Identifier: Apache-2.0
//! A `SQLite` connection as a driver [`Session`]: statements run on the
//! connection thread and stream back; cancel interrupts the running step.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use openrows_driver_api::{
    BatchPolicy, BoxFuture, CancelHandle, Catalog, Connection, DriverError, Editor, ExecSummary,
    ResultSink, ServerSummary, Session, SinkFlow,
};
use rusqlite::InterruptHandle;
use tokio::sync::mpsc;

use crate::catalog::sql as checked;
use crate::stream::{self, Event, Job};
use crate::worker::Worker;

/// Batches the connection thread may run ahead of the sink.
const AHEAD: usize = 2;

/// One open database file, owned by a single session actor (ADR-0007).
pub struct SqliteSession {
    pub(crate) worker: Worker,
    interrupt: Arc<InterruptHandle>,
    summary: ServerSummary,
    policy: BatchPolicy,
}

impl SqliteSession {
    pub(crate) fn new(worker: Worker, interrupt: InterruptHandle, summary: ServerSummary) -> Self {
        Self {
            worker,
            interrupt: Arc::new(interrupt),
            summary,
            policy: BatchPolicy::default(),
        }
    }

    /// Run the profile's startup commands in order, stepping through any
    /// rows; the first failure names its command.
    ///
    /// # Errors
    /// The failing command's `Server` error.
    pub(crate) async fn run_startup_commands(
        &mut self,
        commands: Vec<String>,
    ) -> Result<(), DriverError> {
        self.worker
            .call(move |connection| {
                commands.iter().enumerate().try_for_each(|(index, sql)| {
                    startup_command(connection, sql)
                        .map_err(|error| error.in_startup_command(index))
                })
            })
            .await?
    }

    async fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        let job = Job {
            sql: sql.to_owned(),
            params: params.to_vec(),
            policy: self.policy,
        };
        let (events, mut received) = mpsc::channel(AHEAD);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let done = self
            .worker
            .call(move |connection| stream::run(connection, &job, &events, &worker_stop));
        let forward = async {
            while let Some(event) = received.recv().await {
                deliver(event, sink, &stop);
            }
        };
        let (result, ()) = tokio::join!(done, forward);
        result?
    }
}

/// Hand one event to the sink; batches already in flight when the sink
/// stopped are dropped.
fn deliver(event: Event, sink: &mut dyn ResultSink, stop: &AtomicBool) {
    if stop.load(Ordering::SeqCst) {
        return;
    }
    match event {
        Event::Begin(schema) => sink.begin_result(schema),
        Event::Batch(batch) => {
            if sink.push_batch(batch) == SinkFlow::Stop {
                stop.store(true, Ordering::SeqCst);
            }
        }
    }
}

impl Session for SqliteSession {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run(sql, &[], sink))
    }

    fn execute_bound<'a>(
        &'a mut self,
        sql: &'a str,
        params: &'a [Option<String>],
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run(sql, params, sink))
    }

    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        Some(Arc::new(Interrupt(Arc::clone(&self.interrupt))))
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        // Dropping the worker ends the thread, which closes the file.
        drop(self);
        Box::pin(async { Ok(()) })
    }
}

impl Connection for SqliteSession {
    fn server(&self) -> &ServerSummary {
        &self.summary
    }

    fn catalog(&mut self) -> Option<&mut dyn Catalog> {
        Some(self)
    }

    fn editor(&mut self) -> Option<&mut dyn Editor> {
        Some(self)
    }
}

/// `sqlite3_interrupt`: the running step fails with `SQLITE_INTERRUPT`
/// (SQLSTATE 57014) and the connection stays usable.
struct Interrupt(Arc<InterruptHandle>);

impl CancelHandle for Interrupt {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        self.0.interrupt();
        Box::pin(async { Ok(()) })
    }
}

fn startup_command(connection: &rusqlite::Connection, sql: &str) -> Result<(), DriverError> {
    let mut statement = checked(connection.prepare(sql))?;
    let mut rows = statement.raw_query();
    while checked(rows.next())?.is_some() {}
    Ok(())
}
