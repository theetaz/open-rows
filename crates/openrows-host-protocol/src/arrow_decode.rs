// SPDX-License-Identifier: Apache-2.0
//! Preflight each IPC segment before allocating arrays or retaining dictionaries.
use crate::{BATCH_LIMIT, Grant, LARGE_LIMIT, ProtocolError, arrow_validate::Validation};
use arrow_array::{ArrayRef, RecordBatch};
use arrow_buffer::Buffer;
use arrow_ipc::{
    MessageHeader,
    convert::try_fb_to_schema,
    reader::{read_dictionary, read_record_batch},
    root_as_message,
};
use arrow_schema::SchemaRef;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Incremental bounded result decoder with exact schema/extension metadata.
pub struct ArrowDecoder {
    schema: Option<SchemaRef>,
    dictionaries: HashMap<i64, ArrayRef>,
    validation: Validation,
    ended: bool,
}
impl Default for ArrowDecoder {
    fn default() -> Self {
        Self::new()
    }
}
impl ArrowDecoder {
    /// Start one result; dictionaries never cross result boundaries.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema: None,
            dictionaries: HashMap::new(),
            validation: Validation::default(),
            ended: false,
        }
    }
    /// Decode only one prior-admitted batch or schema-only segment.
    /// # Errors
    /// Invalid metadata, dictionary expansion, decoded budget or IPC body.
    pub fn admit(
        &mut self,
        mut bytes: &[u8],
        grant: Grant,
    ) -> Result<Option<RecordBatch>, ProtocolError> {
        let limit = if grant.exclusive {
            LARGE_LIMIT
        } else {
            BATCH_LIMIT
        };
        if self.ended
            || grant.encoded == 0
            || grant.decoded == 0
            || bytes.len() > grant.encoded as usize
            || grant.decoded as usize > limit
            || bytes.len() > limit
        {
            return Err(ProtocolError::Limit);
        }
        self.validation.check(bytes, grant.decoded as usize)?;
        let mut result = None;
        while !bytes.is_empty() {
            let size =
                u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| ProtocolError::Invalid)?)
                    as usize;
            let message =
                root_as_message(&bytes[8..8 + size]).map_err(|_| ProtocolError::Invalid)?;
            let body_size =
                usize::try_from(message.bodyLength()).map_err(|_| ProtocolError::Invalid)?;
            // A dictionary must not retain the current record batch's entire body.
            let body = Buffer::from(&bytes[8 + size..8 + size + body_size]);
            let decoded = self.message(message, &body)?;
            if decoded.is_some() && result.is_some() {
                return Err(ProtocolError::Invalid);
            }
            if decoded.is_some() {
                result = decoded;
            }
            bytes = &bytes[8 + size + body_size..];
        }
        if self.memory(result.as_ref()) > grant.decoded as usize {
            return Err(ProtocolError::Limit);
        }
        Ok(result)
    }
    fn message(
        &mut self,
        message: arrow_ipc::Message<'_>,
        body: &Buffer,
    ) -> Result<Option<RecordBatch>, ProtocolError> {
        match message.header_type() {
            MessageHeader::Schema => {
                if self.schema.is_some() {
                    return Err(ProtocolError::Invalid);
                }
                let schema = message.header_as_schema().ok_or(ProtocolError::Invalid)?;
                self.schema = Some(Arc::new(
                    try_fb_to_schema(schema).map_err(|_| ProtocolError::Invalid)?,
                ));
                Ok(None)
            }
            MessageHeader::DictionaryBatch => {
                let schema = self.schema.as_ref().ok_or(ProtocolError::Invalid)?;
                let dictionary = message
                    .header_as_dictionary_batch()
                    .ok_or(ProtocolError::Invalid)?;
                read_dictionary(
                    body,
                    dictionary,
                    schema,
                    &mut self.dictionaries,
                    &message.version(),
                )
                .map_err(|_| ProtocolError::Invalid)?;
                Ok(None)
            }
            MessageHeader::RecordBatch => {
                let schema = self.schema.clone().ok_or(ProtocolError::Invalid)?;
                let batch = message
                    .header_as_record_batch()
                    .ok_or(ProtocolError::Invalid)?;
                read_record_batch(
                    body,
                    batch,
                    schema,
                    &self.dictionaries,
                    None,
                    &message.version(),
                )
                .map(Some)
                .map_err(|_| ProtocolError::Invalid)
            }
            _ => Err(ProtocolError::Invalid),
        }
    }
    fn memory(&self, batch: Option<&RecordBatch>) -> usize {
        let mut pointers = HashSet::new();
        let mut bytes = self.schema.as_ref().map_or(0, |schema| {
            schema
                .fields()
                .iter()
                .map(|field| field.size())
                .sum::<usize>()
                + 128
        });
        for dictionary in self.dictionaries.values() {
            bytes += crate::memory::data_memory(&dictionary.to_data(), &mut pointers);
        }
        if let Some(batch) = batch {
            for array in batch.columns() {
                bytes += crate::memory::data_memory(&array.to_data(), &mut pointers);
            }
        }
        bytes
    }
    /// Require exact EOS after the final authorized batch.
    /// # Errors
    /// Missing, malformed, duplicate EOS or missing schema.
    pub fn finish(&mut self, bytes: &[u8]) -> Result<(), ProtocolError> {
        if self.ended || self.schema.is_none() || bytes != [255, 255, 255, 255, 0, 0, 0, 0] {
            return Err(ProtocolError::Truncated);
        }
        self.ended = true;
        Ok(())
    }
    /// Exact field/schema metadata after the first admitted segment.
    #[must_use]
    pub fn schema(&self) -> Option<SchemaRef> {
        self.schema.clone()
    }
}
