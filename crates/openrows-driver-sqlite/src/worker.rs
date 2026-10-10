// SPDX-License-Identifier: Apache-2.0
//! The dedicated thread that owns one `SQLite` connection (docs/05 §4):
//! `SQLite` calls block, so they never run on the async runtime. Jobs run in
//! order; dropping the [`Worker`] ends the thread and closes the file.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;

use openrows_driver_api::DriverError;
use rusqlite::Connection;
use tokio::sync::oneshot;

type Job = Box<dyn FnOnce(&mut Connection) + Send>;

/// Handle to the connection thread.
pub(crate) struct Worker {
    jobs: mpsc::Sender<Job>,
    transaction: Arc<AtomicBool>,
}

impl Worker {
    /// Move `connection` onto a new thread.
    ///
    /// # Errors
    /// `Protocol` when the OS refuses to start the thread.
    pub(crate) fn spawn(mut connection: Connection) -> Result<Self, DriverError> {
        let transaction = Arc::new(AtomicBool::new(!connection.is_autocommit()));
        let (jobs, queue) = mpsc::channel::<Job>();
        thread::Builder::new()
            .name("openrows-sqlite".to_owned())
            .spawn(move || {
                while let Ok(job) = queue.recv() {
                    job(&mut connection);
                }
            })
            .map_err(|error| DriverError::Protocol(format!("SQLite thread: {error}")))?;
        Ok(Self { jobs, transaction })
    }

    /// Current transaction state, published before each operation reply.
    pub(crate) fn in_transaction(&self) -> bool {
        self.transaction.load(Ordering::SeqCst)
    }

    /// Run `work` on the connection thread and await its result.
    ///
    /// # Errors
    /// `Protocol` when the thread has stopped (it only stops when dropped).
    pub(crate) async fn call<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Connection) -> T + Send + 'static,
    ) -> Result<T, DriverError> {
        let (reply, answer) = oneshot::channel();
        let transaction = self.transaction.clone();
        let job: Job = Box::new(move |connection| {
            // The caller may have stopped waiting; the work still ran.
            let result = work(connection);
            transaction.store(!connection.is_autocommit(), Ordering::SeqCst);
            let _ = reply.send(result);
        });
        self.jobs.send(job).map_err(|_| stopped())?;
        answer.await.map_err(|_| stopped())
    }
}

fn stopped() -> DriverError {
    DriverError::Protocol("the SQLite connection thread stopped".to_owned())
}
