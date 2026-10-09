// SPDX-License-Identifier: Apache-2.0
use std::borrow::Cow;

use arrow_array::RecordBatch;

use super::*;
use crate::tds::types::{Layout, SqlType, TypeInfo};

#[derive(Default)]
struct Recorder {
    schemas: Vec<SchemaRef>,
    batches: Vec<RecordBatch>,
    notices: Vec<(NoticeLevel, String)>,
}

impl ResultSink for Recorder {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.schemas.push(schema);
    }

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.batches.push(batch);
        SinkFlow::Continue
    }

    fn notice(&mut self, level: NoticeLevel, message: &str) {
        self.notices.push((level, message.to_owned()));
    }
}

fn columns(sql: SqlType) -> Arc<[Column]> {
    Arc::from(vec![Column {
        name: "c".to_owned(),
        info: TypeInfo {
            sql,
            layout: Layout::ByteLen,
        },
        user_type: 0,
    }])
}

fn stamp(days: u32) -> Vec<u8> {
    let mut bytes = vec![0; 5];
    bytes.extend(&days.to_le_bytes()[..3]);
    bytes
}

#[test]
fn a_fallback_after_the_first_batch_continues_in_a_new_result() -> Result<(), DriverError> {
    let mut sink = Recorder::default();
    let policy = BatchPolicy {
        first_batch_rows: 1,
        target_cells: 1,
        max_bytes: 1 << 20,
    };
    let mut rows = ArrowRows::new(&mut sink, policy);
    rows.begin(&columns(SqlType::DateTime2 { scale: 7 }))?;
    rows.row(vec![Some(Cow::Owned(stamp(719_162)))])?;
    rows.row(vec![Some(Cow::Owned(stamp(3_652_058)))])?;
    rows.end()?;
    rows.counted(Count {
        rows: 1,
        result: false,
    });
    rows.counted(Count {
        rows: 2,
        result: true,
    });
    rows.notice(&Message {
        number: 0,
        text: "hi".into(),
        ..Message::default()
    });
    let types: Vec<_> = sink
        .schemas
        .iter()
        .map(|s| s.field(0).data_type().clone())
        .collect();
    assert_eq!(types.len(), 2);
    assert_eq!(types[1], arrow_schema::DataType::Utf8);
    let notices: Vec<_> = sink.notices.iter().map(|(_, text)| text.as_str()).collect();
    assert_eq!(notices, [CONTINUED, "(1 row affected)", "hi"]);
    Ok(())
}

#[test]
fn empty_results_are_published_and_text_rows_render() -> Result<(), DriverError> {
    let mut sink = Recorder::default();
    let mut rows = ArrowRows::new(&mut sink, BatchPolicy::default());
    rows.begin(&columns(SqlType::Int))?;
    rows.end()?;
    assert_eq!(sink.schemas.len(), 1);
    let mut text = TextRows::default();
    text.begin(&columns(SqlType::Int))?;
    text.row(vec![Some(Cow::Owned(9_i32.to_le_bytes().to_vec()))])?;
    text.row(vec![None])?;
    assert_eq!(text.rows, [vec![Some("9".to_owned())], vec![None]]);
    Ok(())
}
