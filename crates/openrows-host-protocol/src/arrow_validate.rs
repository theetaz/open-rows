// SPDX-License-Identifier: Apache-2.0
//! Validate flatbuffer lengths, nesting and dictionary allocations before decode.
use crate::{CONTROL_LIMIT, ProtocolError};
use arrow_ipc::{MessageHeader, MetadataVersion, root_as_message};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Validation {
    dictionaries: BTreeMap<i64, usize>,
    fields: usize,
}
impl Validation {
    pub(crate) fn check(&mut self, mut bytes: &[u8], limit: usize) -> Result<(), ProtocolError> {
        while !bytes.is_empty() {
            let prefix = bytes.get(..8).ok_or(ProtocolError::Truncated)?;
            if prefix[..4] != [255; 4] {
                return Err(ProtocolError::Invalid);
            }
            let size =
                u32::from_le_bytes(prefix[4..].try_into().map_err(|_| ProtocolError::Invalid)?)
                    as usize;
            if size == 0 || size > CONTROL_LIMIT {
                return Err(ProtocolError::Limit);
            }
            let metadata = bytes.get(8..8 + size).ok_or(ProtocolError::Truncated)?;
            let message = root_as_message(metadata).map_err(|_| ProtocolError::Invalid)?;
            if message.version() != MetadataVersion::V5 {
                return Err(ProtocolError::Incompatible);
            }
            let body = usize::try_from(message.bodyLength()).map_err(|_| ProtocolError::Invalid)?;
            if body > limit || bytes.len() < 8 + size + body {
                return Err(ProtocolError::Limit);
            }
            self.message(message, limit)?;
            bytes = &bytes[8 + size + body..];
        }
        Ok(())
    }
    fn check_schema(&mut self, schema: arrow_ipc::Schema<'_>) -> Result<(), ProtocolError> {
        self.fields = 0;
        if let Some(fields) = schema.fields() {
            for field in fields {
                check_field(field, 0, &mut self.fields)?;
            }
        }
        Ok(())
    }
    fn message(
        &mut self,
        message: arrow_ipc::Message<'_>,
        limit: usize,
    ) -> Result<(), ProtocolError> {
        match message.header_type() {
            MessageHeader::Schema => {
                self.check_schema(message.header_as_schema().ok_or(ProtocolError::Invalid)?)
            }
            MessageHeader::RecordBatch => {
                let batch = message
                    .header_as_record_batch()
                    .ok_or(ProtocolError::Invalid)?;
                check_batch(batch, message.bodyLength(), limit)?;
                Ok(())
            }
            MessageHeader::DictionaryBatch => {
                let dictionary = message
                    .header_as_dictionary_batch()
                    .ok_or(ProtocolError::Invalid)?;
                let data = dictionary.data().ok_or(ProtocolError::Invalid)?;
                let cost = check_batch(data, message.bodyLength(), limit)?;
                let previous = self
                    .dictionaries
                    .get(&dictionary.id())
                    .copied()
                    .unwrap_or_default();
                let cost = if dictionary.isDelta() {
                    cost.saturating_add(previous)
                } else {
                    cost
                };
                if cost > limit || self.dictionaries.len() >= 1024 {
                    return Err(ProtocolError::Limit);
                }
                self.dictionaries.insert(dictionary.id(), cost);
                if self.dictionaries.values().sum::<usize>() > limit {
                    return Err(ProtocolError::Limit);
                }
                Ok(())
            }
            _ => Err(ProtocolError::Invalid),
        }
    }
}
fn check_field(
    field: arrow_ipc::Field<'_>,
    depth: usize,
    count: &mut usize,
) -> Result<(), ProtocolError> {
    *count += 1;
    if depth > 16 || *count > 1024 {
        return Err(ProtocolError::Limit);
    }
    if let Some(children) = field.children() {
        for child in children {
            check_field(child, depth + 1, count)?;
        }
    }
    Ok(())
}
fn check_batch(
    batch: arrow_ipc::RecordBatch<'_>,
    body: i64,
    limit: usize,
) -> Result<usize, ProtocolError> {
    if batch.compression().is_some() {
        return Err(ProtocolError::Invalid);
    }
    let rows = usize::try_from(batch.length()).map_err(|_| ProtocolError::Invalid)?;
    if rows > limit / 8 {
        return Err(ProtocolError::Limit);
    }
    let mut estimated = usize::try_from(body).map_err(|_| ProtocolError::Invalid)?;
    if let Some(nodes) = batch.nodes() {
        if nodes.len() > 4096 {
            return Err(ProtocolError::Limit);
        }
        for node in nodes {
            let count = usize::try_from(node.length()).map_err(|_| ProtocolError::Invalid)?;
            if count > limit / 8 || node.null_count() < 0 || node.null_count() > node.length() {
                return Err(ProtocolError::Limit);
            }
            estimated = estimated.saturating_add(64);
        }
    }
    if let Some(buffers) = batch.buffers() {
        if buffers.len() > 16384 {
            return Err(ProtocolError::Limit);
        }
        for buffer in buffers {
            if buffer.offset() < 0
                || buffer.length() < 0
                || buffer
                    .offset()
                    .checked_add(buffer.length())
                    .is_none_or(|end| end > body)
            {
                return Err(ProtocolError::Invalid);
            }
        }
    }
    if estimated > limit {
        Err(ProtocolError::Limit)
    } else {
        Ok(estimated)
    }
}
