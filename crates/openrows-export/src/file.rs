// SPDX-License-Identifier: Apache-2.0
//! A buffered temporary file next to the destination, renamed into place
//! only on commit and deleted when dropped uncommitted.
use crate::ExportError;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Write buffer: large sequential writes, few syscalls.
const BUFFER: usize = 256 * 1024;
/// Distinguishes temporary files of concurrent exports in one process.
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
/// Temporary names tried before giving up on a directory full of leftovers.
const ATTEMPTS: usize = 64;

pub(crate) struct ExportFile {
    writer: Option<BufWriter<File>>,
    temporary: PathBuf,
    target: PathBuf,
}

impl ExportFile {
    /// Create `.<name>.<pid>-<n>.part` in the destination's directory, so the
    /// final rename stays on one file system and is atomic.
    pub(crate) fn create(target: &Path) -> Result<Self, ExportError> {
        let name = target.file_name().ok_or(ExportError::InvalidPath)?;
        let directory = target
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        for _ in 0..ATTEMPTS {
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let mut temporary_name = std::ffi::OsString::from(".");
            temporary_name.push(name);
            temporary_name.push(format!(".{}-{sequence}.part", std::process::id()));
            let temporary = directory.join(temporary_name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => {
                    return Ok(Self {
                        writer: Some(BufWriter::with_capacity(BUFFER, file)),
                        temporary,
                        target: target.to_path_buf(),
                    });
                }
                // A leftover from an earlier run: try the next sequence number.
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(std::io::Error::from(ErrorKind::AlreadyExists).into())
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), ExportError> {
        match self.writer.as_mut() {
            Some(writer) => writer.write_all(bytes).map_err(Into::into),
            None => Err(ExportError::Aborted),
        }
    }

    /// Flush, sync and rename over the destination.
    pub(crate) fn commit(mut self) -> Result<(), ExportError> {
        let writer = self.writer.take().ok_or(ExportError::Aborted)?;
        let file = writer
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&self.temporary, &self.target)?;
        // Renamed: nothing left for `drop` to delete.
        self.temporary = PathBuf::new();
        Ok(())
    }
}

impl Drop for ExportFile {
    fn drop(&mut self) {
        drop(self.writer.take());
        if !self.temporary.as_os_str().is_empty() {
            // Best effort: the file may already be gone.
            let _ = std::fs::remove_file(&self.temporary);
        }
    }
}
