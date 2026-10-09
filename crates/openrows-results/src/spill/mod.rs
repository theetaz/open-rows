// SPDX-License-Identifier: Apache-2.0
//! Spilling cold batches to disk (docs/06 §3).
//!
//! A pass runs in three phases so readers never wait on disk I/O: victims
//! are picked under the store's read lock, written with no lock held, and
//! swapped in under the write lock, where a result released or replaced in
//! the meantime is skipped (its new file is deleted when dropped).
mod dir;
mod file;
mod plan;
mod worker;

pub use dir::SpillDir;
pub(crate) use file::SpillFile;
pub(crate) use plan::{Victim, pick};

use crate::store::WindowError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use worker::Signal;

/// The spill side of a store: directory, thread signal and health.
pub(crate) struct Spill {
    dir: SpillDir,
    signal: Arc<Signal>,
    next_file: AtomicU64,
    /// Set after a failed write or on close; the store then stops streams
    /// at the memory budget instead of spilling.
    broken: AtomicBool,
}

impl Spill {
    /// Spill into `dir`; `pass` runs on the spill thread per request.
    pub(crate) fn start(dir: SpillDir, pass: impl Fn() + Send + 'static) -> std::io::Result<Self> {
        let signal = Arc::new(Signal::default());
        worker::spawn(Arc::clone(&signal), pass)?;
        Ok(Self {
            dir,
            signal,
            next_file: AtomicU64::new(0),
            broken: AtomicBool::new(false),
        })
    }

    /// Whether over-budget batches can still go to disk.
    pub(crate) fn available(&self) -> bool {
        !self.broken.load(Ordering::Relaxed)
    }

    /// Wake the spill thread; returns at once.
    pub(crate) fn request(&self) {
        self.signal.request();
    }

    /// Block until the spill thread is idle.
    pub(crate) fn wait_idle(&self) {
        self.signal.wait_idle();
    }

    /// Write one victim's batches to a new file.
    ///
    /// # Errors
    /// I/O or encoding errors; spilling is then disabled for this run.
    pub(crate) fn write(&self, victim: &Victim) -> Result<Arc<SpillFile>, WindowError> {
        let id = self.next_file.fetch_add(1, Ordering::Relaxed);
        let batches = victim.batches.iter().map(|(_, batch)| batch);
        SpillFile::write(self.dir.path(), id, &victim.schema, batches)
            .map(Arc::new)
            .inspect_err(|_| self.broken.store(true, Ordering::Relaxed))
    }

    /// Stop spilling and delete the run directory with every file in it.
    pub(crate) fn close(&self) {
        self.broken.store(true, Ordering::Relaxed);
        self.signal.close();
        self.dir.remove();
    }
}

impl Drop for Spill {
    fn drop(&mut self) {
        self.signal.close();
    }
}

#[cfg(test)]
pub(crate) use dir::sweep;
