// SPDX-License-Identifier: Apache-2.0
//! Bounded asynchronous HTTP chunks exposed as a blocking serde Read.
use bytes::Bytes;
use std::io::{self, Read};
use tokio::sync::mpsc;

pub(crate) struct ChunkReader {
    chunks: mpsc::Receiver<Bytes>,
    current: Bytes,
}

impl ChunkReader {
    pub(crate) fn new(chunks: mpsc::Receiver<Bytes>) -> Self {
        Self {
            chunks,
            current: Bytes::new(),
        }
    }
}

impl Read for ChunkReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        while self.current.is_empty() {
            let Some(chunk) = self.chunks.blocking_recv() else {
                return Ok(0);
            };
            self.current = chunk;
        }
        let count = buffer.len().min(self.current.len());
        buffer[..count].copy_from_slice(&self.current[..count]);
        self.current = self.current.slice(count..);
        Ok(count)
    }
}
