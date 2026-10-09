// SPDX-License-Identifier: Apache-2.0
//! JSON lines to a daily rotating file, every line scrubbed before it is written.
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tracing::Subscriber;
use tracing::level_filters::LevelFilter;
use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};
use tracing_subscriber::fmt::MakeWriter;

use super::scrub::scrub;

/// Log files kept on disk, the newest included (docs/14 §6: 7 days).
pub(crate) const KEPT_FILES: usize = 7;

/// File name prefix; files are named `openrows.YYYY-MM-DD.log`.
pub(crate) const PREFIX: &str = "openrows";

/// Writes complete lines through [`scrub`]; a partial line waits for its end.
pub(crate) struct Scrubbing<W: Write> {
    inner: W,
    pending: Vec<u8>,
}

impl<W: Write> Scrubbing<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            pending: Vec::new(),
        }
    }

    fn write_complete_lines(&mut self) -> io::Result<()> {
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            self.write_scrubbed(&line)?;
        }
        Ok(())
    }

    fn write_pending(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let rest = std::mem::take(&mut self.pending);
        self.write_scrubbed(&rest)
    }

    fn write_scrubbed(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.inner
            .write_all(scrub(&String::from_utf8_lossy(bytes)).as_bytes())
    }
}

impl<W: Write> Write for Scrubbing<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        self.write_complete_lines()?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write_pending()?;
        self.inner.flush()
    }
}

impl<W: Write> Drop for Scrubbing<W> {
    fn drop(&mut self) {
        // `drop` cannot report an error; the unfinished line is best effort.
        let _ = self.write_pending();
    }
}

/// One output shared by every event; each event locks it while it is written.
pub(crate) struct Shared<T>(Arc<Mutex<T>>);

impl<T> Shared<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

/// Lock a shared output; a panic while it was held must not silence logging.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
impl Shared<Vec<u8>> {
    /// Everything written so far.
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&lock(&self.0)).into_owned()
    }
}

/// The lock held while one event is written.
pub(crate) struct Locked<'a, T>(MutexGuard<'a, T>);

impl<T: Write> Write for Locked<'_, T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a, T: Write + Send + 'static> MakeWriter<'a> for Shared<T> {
    type Writer = Scrubbing<Locked<'a, T>>;

    fn make_writer(&'a self) -> Self::Writer {
        Scrubbing::new(Locked(lock(&self.0)))
    }
}

/// A JSON-lines subscriber at `level` writing through `writer`.
pub(crate) fn subscriber<M>(writer: M, level: LevelFilter) -> impl Subscriber + Send + Sync
where
    M: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .with_writer(writer)
        .with_max_level(level)
        .finish()
}

/// The daily rotating log file in `dir`, keeping [`KEPT_FILES`] files.
///
/// # Errors
/// The directory cannot be read or the first file cannot be created.
pub(crate) fn rolling(
    dir: &Path,
    level: LevelFilter,
) -> Result<impl Subscriber + Send + Sync, InitError> {
    let file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(PREFIX)
        .filename_suffix("log")
        .max_log_files(KEPT_FILES)
        .build(dir)?;
    Ok(subscriber(Shared::new(file), level))
}
