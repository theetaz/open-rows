#![allow(clippy::unwrap_used, reason = "synthetic fixtures and test assertions")]
// SPDX-License-Identifier: Apache-2.0
//! Arrow values, dictionaries and extension metadata cross the process wire intact.
use arrow_array::{
    ArrayRef, BinaryArray, Decimal128Array, Int64Array, RecordBatch, StringArray,
    builder::StringDictionaryBuilder, types::Int8Type,
};
use arrow_schema::{DataType, Field, Schema};
use openrows_host_protocol::*;
use std::{collections::HashMap, sync::Arc};

fn fixture() -> RecordBatch {
    let mut dictionary = StringDictionaryBuilder::<Int8Type>::new();
    dictionary.append("test-only").unwrap();
    dictionary.append_null();
    dictionary.append("test-only").unwrap();
    let arrays: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from(vec![Some(i64::MIN), None, Some(i64::MAX)])),
        Arc::new(
            Decimal128Array::from(vec![Some(1_234_567_890_123_456_789), None, Some(-3)])
                .with_precision_and_scale(30, 8)
                .unwrap(),
        ),
        Arc::new(StringArray::from(vec![Some("ම test-only"), None, Some("")])),
        Arc::new(BinaryArray::from(vec![
            Some(&b"\0\xfftest-only"[..]),
            None,
            Some(&b""[..]),
        ])),
        Arc::new(dictionary.finish()),
    ];
    let fields: Vec<_> = arrays
        .iter()
        .enumerate()
        .map(|(index, array)| {
            Field::new(format!("c{index}"), array.data_type().clone(), true).with_metadata(
                HashMap::from([
                    ("ARROW:extension:name".into(), "openrows.test-only".into()),
                    ("ARROW:extension:metadata".into(), "test-only".into()),
                ]),
            )
        })
        .collect();
    RecordBatch::try_new(
        Arc::new(Schema::new_with_metadata(
            fields,
            HashMap::from([("test-only".into(), "schema".into())]),
        )),
        arrays,
    )
    .unwrap()
}
fn grant(encoded: &EncodedBatch) -> Grant {
    Grant {
        key: FlowKey {
            request: 1,
            result: 1,
            batch: 1,
        },
        encoded: u32::try_from(encoded.bytes.len()).unwrap(),
        decoded: u32::try_from(encoded.decoded).unwrap(),
        exclusive: false,
    }
}
#[test]
fn arrow_stream_round_trip_preserves_exact_values_types_nulls_dictionaries_metadata()
-> Result<(), ProtocolError> {
    let batch = fixture();
    let mut writer = ArrowEncoder::new(batch.schema())?;
    let mut reader = ArrowDecoder::new();
    for _ in 0..3 {
        let encoded = writer.batch(&batch)?;
        let decoded = reader.admit(&encoded.bytes, grant(&encoded))?.unwrap();
        assert_eq!(decoded, batch);
        assert_eq!(reader.schema().unwrap(), batch.schema());
    }
    reader.finish(&writer.finish()?)?;
    assert_eq!(
        reader.finish(&[255, 255, 255, 255, 0, 0, 0, 0]),
        Err(ProtocolError::Truncated)
    );
    Ok(())
}
#[test]
fn malformed_truncated_arrow_and_dictionary_expansion_fail_before_decode()
-> Result<(), ProtocolError> {
    let batch = fixture();
    let mut writer = ArrowEncoder::new(batch.schema())?;
    let encoded = writer.batch(&batch)?;
    assert!(matches!(
        ArrowDecoder::new().admit(&encoded.bytes[..encoded.bytes.len() - 1], grant(&encoded)),
        Err(ProtocolError::Limit | ProtocolError::Truncated)
    ));
    let mut broken = encoded.bytes.clone();
    broken[8..12].fill(255);
    assert_eq!(
        ArrowDecoder::new().admit(&broken, grant(&encoded)),
        Err(ProtocolError::Invalid)
    );
    // This advertises millions of dictionary entries in a tiny flatbuffer.
    let huge = dictionary_blowup();
    let offered = Grant {
        encoded: u32::try_from(huge.len()).unwrap(),
        decoded: u32::try_from(BATCH_LIMIT).unwrap(),
        ..grant(&encoded)
    };
    assert_eq!(
        ArrowDecoder::new().admit(&huge, offered),
        Err(ProtocolError::Limit)
    );
    Ok(())
}
fn dictionary_blowup() -> Vec<u8> {
    let mut builder = flatbuffers::FlatBufferBuilder::new();
    let nodes = builder.create_vector(&[arrow_ipc::FieldNode::new(i64::MAX, 0)]);
    let batch = arrow_ipc::RecordBatch::create(
        &mut builder,
        &arrow_ipc::RecordBatchArgs {
            length: i64::MAX,
            nodes: Some(nodes),
            ..Default::default()
        },
    );
    let dictionary = arrow_ipc::DictionaryBatch::create(
        &mut builder,
        &arrow_ipc::DictionaryBatchArgs {
            id: 0,
            data: Some(batch),
            isDelta: true,
        },
    );
    let message = arrow_ipc::Message::create(
        &mut builder,
        &arrow_ipc::MessageArgs {
            version: arrow_ipc::MetadataVersion::V5,
            header_type: arrow_ipc::MessageHeader::DictionaryBatch,
            header: Some(dictionary.as_union_value()),
            bodyLength: 0,
            ..Default::default()
        },
    );
    builder.finish(message, None);
    let metadata = builder.finished_data();
    let length = metadata.len().next_multiple_of(8);
    let mut bytes = vec![255; 4];
    bytes.extend_from_slice(&(u32::try_from(length).unwrap()).to_le_bytes());
    bytes.extend_from_slice(metadata);
    bytes.resize(8 + length, 0);
    bytes
}
#[test]
fn empty_schema_stream_still_requires_admission_and_eos() -> Result<(), ProtocolError> {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "empty",
        DataType::Int64,
        true,
    )]));
    let mut writer = ArrowEncoder::new(schema.clone())?;
    let encoded = writer.schema_only();
    let mut reader = ArrowDecoder::new();
    assert!(reader.admit(&encoded.bytes, grant(&encoded))?.is_none());
    assert_eq!(reader.schema(), Some(schema));
    reader.finish(&writer.finish()?)
}
#[test]
fn exclusive_large_rows_and_explicit_ceiling_never_truncate() -> Result<(), ProtocolError> {
    let schema = Arc::new(Schema::new(vec![Field::new(
        "large",
        DataType::Utf8,
        false,
    )]));
    let text = "x".repeat(BATCH_LIMIT + 128);
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(StringArray::from(vec![text.as_str()]))],
    )
    .unwrap();
    let mut writer = ArrowEncoder::new(schema.clone())?;
    let encoded = writer.batch(&batch)?;
    assert!(encoded.bytes.len() > BATCH_LIMIT);
    let mut credit = grant(&encoded);
    credit.exclusive = true;
    assert_eq!(
        ArrowDecoder::new().admit(&encoded.bytes, credit)?.unwrap(),
        batch
    );
    let text = "x".repeat(LARGE_LIMIT + 1);
    let oversized = RecordBatch::try_new(
        schema,
        vec![Arc::new(StringArray::from(vec![text.as_str()]))],
    )
    .unwrap();
    assert!(matches!(split_batch(&oversized), Err(ProtocolError::Limit)));
    Ok(())
}

#[test]
fn multirow_batches_split_lazily_and_compact_retained_backing_buffers() -> Result<(), ProtocolError>
{
    let text = "x".repeat(BATCH_LIMIT / 2 + 128);
    let batch = RecordBatch::try_from_iter(vec![(
        "text",
        Arc::new(StringArray::from(vec![text.as_str(), text.as_str()])) as ArrayRef,
    )])
    .unwrap();
    let mut parts = split_batch(&batch)?;
    for _ in 0..2 {
        let part = parts.next().unwrap()?;
        assert_eq!(part.num_rows(), 1);
        let segment = ArrowEncoder::new(part.schema())?.batch(&part)?;
        assert!(segment.decoded <= BATCH_LIMIT && segment.bytes.len() <= BATCH_LIMIT);
        let mut reader = ArrowDecoder::new();
        assert_eq!(
            reader.admit(&segment.bytes, grant(&segment))?.unwrap(),
            part
        );
    }
    assert!(parts.next().is_none());
    Ok(())
}
