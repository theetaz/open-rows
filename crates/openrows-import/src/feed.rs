// SPDX-License-Identifier: Apache-2.0
//! The reading side of an import: a blocking thread parses the file, maps
//! and checks each row and sends bounded batches over a channel. A full
//! channel blocks the reader (back-pressure), and a dropped receiver (cancel
//! or failure) stops it.
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use openrows_driver_api::TextRow;
use tokio::sync::mpsc;

use crate::mapping::Mapper;
use crate::options::ParseOptions;
use crate::source::{Item, stream};
use crate::{ImportError, RowError};

/// Batches buffered between the reader and the loader.
const CAPACITY: usize = 4;
/// Cell text bytes after which a batch is sent early.
const BATCH_BYTES: usize = 4 * 1024 * 1024;
/// Bad rows after which a batch is sent early.
const BATCH_ERRORS: usize = 1_000;

/// Mapped rows of consecutive lines, with the rows that failed among them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Batch {
    /// Mapped, checked rows in file order.
    pub rows: Vec<TextRow>,
    /// Lines of `rows`, in the same order.
    pub lines: Vec<u64>,
    /// Malformed rows and rejected values among these lines.
    pub errors: Vec<RowError>,
}

impl Batch {
    /// The first and last line of the batch's rows.
    #[must_use]
    pub fn span(&self) -> Option<(u64, u64)> {
        Some((*self.lines.first()?, *self.lines.last()?))
    }
}

/// One message from the reader.
#[derive(Debug)]
pub enum Chunk {
    /// The next rows.
    Batch(Batch),
    /// The file ended (`Ok`) or could not be read further.
    End(Result<(), ImportError>),
}

/// What the reader reads and how it batches.
#[derive(Debug)]
pub struct FeedSource {
    /// The file.
    pub path: PathBuf,
    /// How it is parsed.
    pub options: ParseOptions,
    /// Its columns, as [`crate::inspect`] named them.
    pub columns: Vec<String>,
    /// Source rows to target rows.
    pub mapper: Mapper,
    /// Rows per batch.
    pub batch_rows: usize,
}

/// Start reading on a blocking thread; raw bytes read are added to `read`.
#[must_use]
pub fn spawn_feed(source: FeedSource, read: Arc<AtomicU64>) -> mpsc::Receiver<Chunk> {
    let (sender, receiver) = mpsc::channel(CAPACITY);
    tokio::task::spawn_blocking(move || {
        let ended = feed(&source, read, &sender);
        // The receiver is gone when the import already stopped.
        let _ = sender.blocking_send(Chunk::End(ended));
    });
    receiver
}

fn feed(
    source: &FeedSource,
    read: Arc<AtomicU64>,
    sender: &mpsc::Sender<Chunk>,
) -> Result<(), ImportError> {
    let mut batch = Batch::default();
    let mut bytes = 0;
    let limit = source.batch_rows.max(1);
    let mut gone = false;
    stream(
        &source.path,
        (&source.options, &source.columns),
        read,
        &mut |item| {
            match item {
                Item::Bad(error) => batch.errors.push(error),
                Item::Record(record) => match source.mapper.row(&record) {
                    Ok(row) => {
                        bytes += row.iter().flatten().map(String::len).sum::<usize>();
                        batch.rows.push(row);
                        batch.lines.push(record.line);
                    }
                    Err(error) => batch.errors.push(error),
                },
            }
            let full = batch.rows.len() >= limit
                || bytes >= BATCH_BYTES
                || batch.errors.len() >= BATCH_ERRORS;
            if full {
                bytes = 0;
                if sender
                    .blocking_send(Chunk::Batch(std::mem::take(&mut batch)))
                    .is_err()
                {
                    gone = true;
                    return ControlFlow::Break(());
                }
            }
            ControlFlow::Continue(())
        },
    )?;
    let pending = !batch.rows.is_empty() || !batch.errors.is_empty();
    if pending && !gone {
        // A closed channel means the loader stopped; nothing is left to do.
        let _ = sender.blocking_send(Chunk::Batch(batch));
    }
    Ok(())
}
