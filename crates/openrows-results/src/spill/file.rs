// SPDX-License-Identifier: Apache-2.0
//! One spill file: batches of one result as LZ4-compressed Arrow IPC.
use super::dir::private_file;
use crate::store::WindowError;
use arrow_array::RecordBatch;
use arrow_ipc::CompressionType;
use arrow_ipc::reader::FileReader;
use arrow_ipc::writer::{FileWriter, IpcWriteOptions};
use arrow_schema::{ArrowError, SchemaRef};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// A written spill file; dropping the last handle deletes it, so a file
/// outlives its result's release only while a reader still decodes from it.
#[derive(Debug)]
pub(crate) struct SpillFile {
    id: u64,
    path: PathBuf,
}

impl SpillFile {
    /// Write `batches` to `<dir>/<id>.arrows` (mode 0600).
    ///
    /// # Errors
    /// I/O or Arrow encoding errors; the partial file is deleted.
    pub(crate) fn write<'a>(
        dir: &Path,
        id: u64,
        schema: &SchemaRef,
        batches: impl IntoIterator<Item = &'a RecordBatch>,
    ) -> Result<Self, WindowError> {
        let path = dir.join(format!("{id}.arrows"));
        let file = private_file(&path)?;
        // From here on, dropping `spilled` on an error deletes the file.
        let spilled = Self { id, path };
        let options =
            IpcWriteOptions::default().try_with_compression(Some(CompressionType::LZ4_FRAME))?;
        let mut writer = FileWriter::try_new_with_options(BufWriter::new(file), schema, options)?;
        for batch in batches {
            writer.write(batch)?;
        }
        writer.finish()?;
        writer
            .into_inner()?
            .into_inner()
            .map_err(std::io::IntoInnerError::into_error)?;
        Ok(spilled)
    }

    /// Unique per store; keys the decoded-batch cache.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// Decode batch `index`.
    ///
    /// # Errors
    /// The file is gone or unreadable, or the batch does not exist.
    pub(crate) fn read(&self, index: usize) -> Result<RecordBatch, WindowError> {
        let mut reader = FileReader::try_new_buffered(File::open(&self.path)?, None)?;
        reader.set_index(index)?;
        reader
            .next()
            .ok_or_else(|| {
                WindowError::Arrow(ArrowError::IpcError(format!(
                    "spill file {} has no batch {index}",
                    self.id
                )))
            })?
            .map_err(WindowError::from)
    }
}

impl Drop for SpillFile {
    fn drop(&mut self) {
        // The run directory may already be gone (app exit); nothing to undo.
        let _ = fs::remove_file(&self.path);
    }
}
