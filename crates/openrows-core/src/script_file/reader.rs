// SPDX-License-Identifier: Apache-2.0
//! Statements of a `.sql` file, read in chunks and split as they arrive, so
//! memory holds one chunk and the unfinished statement, never the file.
use openrows_sql::{Dialect, ScriptStatement, StreamSplitter};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;

/// Bytes read per chunk.
const CHUNK: usize = 1024 * 1024;
/// The UTF-8 byte-order mark, skipped at the start of the file.
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Why the file could not be read as a script.
#[derive(Debug, thiserror::Error)]
pub enum ScriptFileError {
    /// Opening or reading the file failed.
    #[error("the file could not be read: {0}")]
    Io(#[from] std::io::Error),
    /// The file is not UTF-8 text.
    #[error("the file is not UTF-8 text (invalid bytes at offset {offset})")]
    NotUtf8 {
        /// Byte offset of the first invalid byte.
        offset: u64,
    },
}

/// Reads the statements of a script file in order; offsets are file offsets.
pub struct ScriptReader {
    file: File,
    splitter: StreamSplitter,
    ready: VecDeque<ScriptStatement>,
    /// Bytes of an incomplete UTF-8 sequence at the end of the last chunk.
    carry: Vec<u8>,
    buffer: Vec<u8>,
    /// Bytes of the file before the splitter's text (the byte-order mark).
    skipped: u64,
    /// Bytes of text given to the splitter.
    fed: u64,
    started: bool,
    ended: bool,
}

impl ScriptReader {
    /// Open `path` for splitting with the rules of `dialect`.
    ///
    /// # Errors
    /// The file cannot be opened.
    pub fn open(path: &Path, dialect: Dialect) -> Result<Self, ScriptFileError> {
        Ok(Self {
            file: File::open(path)?,
            splitter: StreamSplitter::new(dialect),
            ready: VecDeque::new(),
            carry: Vec::new(),
            buffer: vec![0; CHUNK],
            skipped: 0,
            fed: 0,
            started: false,
            ended: false,
        })
    }

    /// Read the next chunk into the splitter; `false` at the end of the file.
    fn fill(&mut self) -> Result<bool, ScriptFileError> {
        let read = loop {
            match self.file.read(&mut self.buffer) {
                Ok(read) => break read,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        };
        if read == 0 {
            if !self.carry.is_empty() {
                return Err(self.invalid(0));
            }
            let found = self.splitter.finish();
            self.queue(found);
            return Ok(false);
        }
        self.carry.extend_from_slice(&self.buffer[..read]);
        if !self.started {
            self.started = true;
            if self.carry.starts_with(BOM) {
                self.carry.drain(..BOM.len());
                self.skipped = BOM.len() as u64;
            }
        }
        let valid = match std::str::from_utf8(&self.carry) {
            Ok(text) => text.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(error) => return Err(self.invalid(error.valid_up_to())),
        };
        let text = std::str::from_utf8(&self.carry[..valid]).map_err(|_| self.invalid(valid))?;
        let found = self.splitter.push(text);
        self.queue(found);
        self.carry.drain(..valid);
        self.fed += valid as u64;
        Ok(true)
    }

    /// Queue statements with offsets in the file (after any byte-order mark).
    fn queue(&mut self, found: Vec<ScriptStatement>) {
        let skipped = self.skipped;
        self.ready.extend(found.into_iter().map(|mut statement| {
            statement.offset += skipped;
            statement.end += skipped;
            statement
        }));
    }

    fn invalid(&self, at: usize) -> ScriptFileError {
        ScriptFileError::NotUtf8 {
            offset: self.skipped + self.fed + at as u64,
        }
    }
}

impl Iterator for ScriptReader {
    type Item = Result<ScriptStatement, ScriptFileError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(statement) = self.ready.pop_front() {
                return Some(Ok(statement));
            }
            if self.ended {
                return None;
            }
            match self.fill() {
                Ok(more) => self.ended = !more,
                Err(error) => {
                    self.ended = true;
                    return Some(Err(error));
                }
            }
        }
    }
}
