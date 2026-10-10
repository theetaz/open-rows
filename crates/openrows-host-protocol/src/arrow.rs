// SPDX-License-Identifier: Apache-2.0
//! Persistent uncompressed Arrow streams with bounded per-batch admission.
use crate::{BATCH_LIMIT, LARGE_LIMIT, ProtocolError};
use arrow_array::RecordBatch;
use arrow_ipc::writer::StreamWriter;
use arrow_schema::SchemaRef;
use std::io::{self, Write};

#[derive(Default)]
struct CappedBytes(Vec<u8>);
impl Write for CappedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > LARGE_LIMIT.saturating_sub(self.0.len()) {
            return Err(io::ErrorKind::OutOfMemory.into());
        }
        let required = self.0.len() + bytes.len();
        if required > self.0.capacity() {
            let target = required.next_power_of_two().min(LARGE_LIMIT);
            self.0
                .try_reserve_exact(target - self.0.len())
                .map_err(|_| io::ErrorKind::OutOfMemory)?;
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// One prepared batch includes schema/dictionary bytes charged to its credit.
pub struct EncodedBatch {
    /// Uncompressed stream segment.
    pub bytes: Vec<u8>,
    /// Checked source Arrow memory, including dictionary values.
    pub decoded: usize,
}
/// Keeps dictionary state across batches in a single result stream.
pub struct ArrowEncoder {
    writer: StreamWriter<CappedBytes>,
    schema: SchemaRef,
}
impl ArrowEncoder {
    /// Begin a stream without sending its schema before a grant.
    /// # Errors
    /// Oversized schema or invalid Arrow.
    pub fn new(schema: SchemaRef) -> Result<Self, ProtocolError> {
        let writer = StreamWriter::try_new(CappedBytes::default(), &schema)
            .map_err(|_| ProtocolError::Invalid)?;
        if writer.get_ref().0.len() > BATCH_LIMIT {
            return Err(ProtocolError::Limit);
        }
        Ok(Self { writer, schema })
    }
    /// Encode one bounded batch; source and serialized buffers overlap only here.
    /// # Errors
    /// Oversized source/encoded memory or schema mismatch.
    pub fn batch(&mut self, batch: &RecordBatch) -> Result<EncodedBatch, ProtocolError> {
        let decoded = crate::memory::batch_memory(batch).saturating_add(self.schema_memory());
        if decoded > LARGE_LIMIT {
            return Err(ProtocolError::Limit);
        }
        self.writer.write(batch).map_err(|_| ProtocolError::Limit)?;
        let bytes = std::mem::take(&mut self.writer.get_mut().0);
        let decoded = decoded.max(
            bytes
                .len()
                .saturating_add(self.schema_memory())
                .saturating_add(batch.num_columns() * 256),
        );
        if decoded > LARGE_LIMIT {
            return Err(ProtocolError::Limit);
        }
        Ok(EncodedBatch {
            bytes,
            decoded: decoded.max(1),
        })
    }
    /// Authorize schema-only empty results using the same credit protocol.
    #[must_use]
    pub fn schema_only(&mut self) -> EncodedBatch {
        EncodedBatch {
            bytes: std::mem::take(&mut self.writer.get_mut().0),
            decoded: self.schema_memory().max(1),
        }
    }
    /// EOS consumes no fresh row credit after the authorized final batch.
    /// # Errors
    /// Invalid writer state.
    pub fn finish(&mut self) -> Result<Vec<u8>, ProtocolError> {
        self.writer.finish().map_err(|_| ProtocolError::Invalid)?;
        Ok(std::mem::take(&mut self.writer.get_mut().0))
    }
    fn schema_memory(&self) -> usize {
        self.schema
            .fields()
            .iter()
            .map(|field| field.size())
            .sum::<usize>()
            .saturating_add(128)
    }
}
pub(crate) fn compact(batch: &RecordBatch) -> Result<RecordBatch, ProtocolError> {
    let mut encoder = ArrowEncoder::new(batch.schema())?;
    encoder
        .writer
        .write(batch)
        .map_err(|_| ProtocolError::Limit)?;
    encoder
        .writer
        .finish()
        .map_err(|_| ProtocolError::Invalid)?;
    let reader = arrow_ipc::reader::StreamReader::try_new(
        io::Cursor::new(&encoder.writer.get_ref().0),
        None,
    )
    .map_err(|_| ProtocolError::Invalid)?;
    reader
        .into_iter()
        .next()
        .ok_or(ProtocolError::Invalid)?
        .map_err(|_| ProtocolError::Invalid)
}
