// SPDX-License-Identifier: Apache-2.0
//! The background spill thread and its wake-up signal.
//!
//! Appends come from session actors, which must not block, so they only
//! raise the signal; one thread per store does the disk writes. Requests
//! raised while a spill runs coalesce into one more pass.
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

#[derive(Default)]
struct State {
    pending: bool,
    running: bool,
    closed: bool,
}

/// Coalescing wake-up between appenders and the spill thread.
#[derive(Default)]
pub(crate) struct Signal {
    state: Mutex<State>,
    changed: Condvar,
}

impl Signal {
    /// Ask for a spill pass; returns at once.
    pub(crate) fn request(&self) {
        self.lock().pending = true;
        self.changed.notify_all();
    }

    /// Stop the thread after its current pass.
    pub(crate) fn close(&self) {
        self.lock().closed = true;
        self.changed.notify_all();
    }

    /// Block until no pass is requested or running.
    pub(crate) fn wait_idle(&self) {
        let mut state = self.lock();
        while (state.pending || state.running) && !state.closed {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Wait for a request; false once closed.
    fn next(&self) -> bool {
        let mut state = self.lock();
        while !state.pending && !state.closed {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.pending = false;
        state.running = !state.closed;
        state.running
    }

    fn done(&self) {
        self.lock().running = false;
        self.changed.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Start the spill thread running `pass` once per (coalesced) request.
/// The pass must hold the store weakly: dropping the store closes the
/// signal, which ends the thread.
pub(crate) fn spawn(signal: Arc<Signal>, pass: impl Fn() + Send + 'static) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("openrows-spill".to_owned())
        .spawn(move || {
            while signal.next() {
                pass();
                signal.done();
            }
        })?;
    Ok(())
}
