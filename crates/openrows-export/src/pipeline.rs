// SPDX-License-Identifier: Apache-2.0
//! The two ways batches reach an [`Exporter`], both for a blocking thread.
use crate::{ExportError, Exporter};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use std::sync::mpsc::Receiver;

/// One step of a streamed result, sent by the producer (a driver sink).
#[derive(Debug)]
pub enum Message {
    /// The result's columns; precedes every batch.
    Begin(SchemaRef),
    /// The next rows.
    Batch(RecordBatch),
    /// The producer delivered every row: commit the file.
    Finish,
}

/// Write streamed messages until [`Message::Finish`]; returns the row count.
///
/// `progress` sees the running row count after every batch. When every
/// sender is dropped before `Finish` (cancel or upstream failure) the result
/// is [`ExportError::Aborted`]; on any error the temporary file is deleted
/// and the receiver dropped, so the producer's next send fails and it stops.
///
/// # Errors
/// [`ExportError::Aborted`], or the first encode or I/O failure.
pub fn drain(
    mut exporter: Exporter,
    messages: Receiver<Message>,
    mut progress: impl FnMut(u64),
) -> Result<u64, ExportError> {
    for message in messages {
        match message {
            Message::Begin(schema) => exporter.begin(schema)?,
            Message::Batch(batch) => {
                exporter.write(&batch)?;
                progress(exporter.rows());
            }
            Message::Finish => return exporter.finish(),
        }
    }
    Err(ExportError::Aborted)
}

/// Write a complete in-memory result, checking `stopped` before each batch.
///
/// # Errors
/// [`ExportError::Aborted`] once `stopped` returns true, or the first
/// encode or I/O failure; the temporary file is deleted either way.
pub fn write_all(
    mut exporter: Exporter,
    (schema, batches): (SchemaRef, &[RecordBatch]),
    stopped: impl Fn() -> bool,
    mut progress: impl FnMut(u64),
) -> Result<u64, ExportError> {
    exporter.begin(schema)?;
    for batch in batches {
        if stopped() {
            return Err(ExportError::Aborted);
        }
        exporter.write(batch)?;
        progress(exporter.rows());
    }
    if stopped() {
        return Err(ExportError::Aborted);
    }
    exporter.finish()
}
