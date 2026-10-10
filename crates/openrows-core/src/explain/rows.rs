// SPDX-License-Identifier: Apache-2.0
//! A [`BatchSink`] keeping the few rows of an explaining statement as text.
use crate::BatchSink;
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Float32Type, Float64Type, Int8Type, Int16Type, Int32Type, Int64Type, UInt8Type, UInt16Type,
    UInt32Type, UInt64Type,
};
use arrow_array::{Array, RecordBatch};
use arrow_schema::{DataType, SchemaRef};
use openrows_driver_api::SinkFlow;
use openrows_types::ResultId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

/// Plans are a handful of rows; anything beyond this is not plan output.
const MAX_ROWS: usize = 10_000;

/// Text cells in column order; `None` is NULL.
pub(crate) type TextRow = Vec<Option<String>>;

/// Collects every row of every result set as text.
#[derive(Default)]
pub(crate) struct TextRows {
    rows: Mutex<Vec<TextRow>>,
    showplan: bool,
    collecting: AtomicBool,
    over_budget: AtomicBool,
    bytes: Mutex<usize>,
}

impl TextRows {
    /// Collect plan output only when SQL Server also emits query results.
    pub(crate) fn new(showplan: bool) -> Self {
        Self {
            showplan,
            ..Self::default()
        }
    }

    /// Whether output exceeded the row or byte bound.
    pub(crate) fn over_budget(&self) -> bool {
        self.over_budget.load(Ordering::Relaxed)
    }

    /// The rows collected so far.
    pub(crate) fn take(&self) -> Vec<TextRow> {
        std::mem::take(&mut *self.rows.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl BatchSink for TextRows {
    fn begin(&self, _result: ResultId, schema: SchemaRef) {
        let collecting = !self.showplan
            || (schema.fields().len() == 1
                && schema.field(0).name() == "Microsoft SQL Server 2005 XML Showplan");
        self.collecting.store(collecting, Ordering::Relaxed);
    }

    fn push(&self, _result: ResultId, batch: RecordBatch) -> SinkFlow {
        if self.showplan && !self.collecting.load(Ordering::Relaxed) {
            return SinkFlow::Continue;
        }
        let mut bytes = self.bytes.lock().unwrap_or_else(PoisonError::into_inner);
        let mut rows = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
        for row in 0..batch.num_rows() {
            if rows.len() >= MAX_ROWS {
                self.over_budget.store(true, Ordering::Relaxed);
                return SinkFlow::Stop;
            }
            let cells: TextRow = batch
                .columns()
                .iter()
                .map(|column| cell(column.as_ref(), row))
                .collect();
            *bytes += cells.iter().flatten().map(String::len).sum::<usize>();
            if *bytes > 8 * 1024 * 1024 {
                self.over_budget.store(true, Ordering::Relaxed);
                return SinkFlow::Stop;
            }
            rows.push(cells);
        }
        SinkFlow::Continue
    }

    fn end(&self, _result: ResultId) {}
}

/// One cell as text: strings, bytes (as UTF-8), booleans and numbers. Plan
/// output uses no other types; anything else reads as NULL.
fn cell(array: &dyn Array, row: usize) -> Option<String> {
    if array.is_null(row) {
        return None;
    }
    let text = match array.data_type() {
        DataType::Utf8 => array.as_string::<i32>().value(row).to_owned(),
        DataType::LargeUtf8 => array.as_string::<i64>().value(row).to_owned(),
        DataType::Utf8View => array.as_string_view().value(row).to_owned(),
        DataType::Binary => {
            String::from_utf8_lossy(array.as_binary::<i32>().value(row)).into_owned()
        }
        DataType::LargeBinary => {
            String::from_utf8_lossy(array.as_binary::<i64>().value(row)).into_owned()
        }
        DataType::Boolean => array.as_boolean().value(row).to_string(),
        _ => return number(array, row),
    };
    Some(text)
}

fn number(array: &dyn Array, row: usize) -> Option<String> {
    let text = match array.data_type() {
        DataType::Int8 => array.as_primitive::<Int8Type>().value(row).to_string(),
        DataType::Int16 => array.as_primitive::<Int16Type>().value(row).to_string(),
        DataType::Int32 => array.as_primitive::<Int32Type>().value(row).to_string(),
        DataType::Int64 => array.as_primitive::<Int64Type>().value(row).to_string(),
        DataType::UInt8 => array.as_primitive::<UInt8Type>().value(row).to_string(),
        DataType::UInt16 => array.as_primitive::<UInt16Type>().value(row).to_string(),
        DataType::UInt32 => array.as_primitive::<UInt32Type>().value(row).to_string(),
        DataType::UInt64 => array.as_primitive::<UInt64Type>().value(row).to_string(),
        DataType::Float32 => array.as_primitive::<Float32Type>().value(row).to_string(),
        DataType::Float64 => array.as_primitive::<Float64Type>().value(row).to_string(),
        _ => return None,
    };
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{BinaryArray, BooleanArray, Int64Array, StringArray};
    use std::sync::Arc;

    #[test]
    fn strings_bytes_booleans_and_integers_become_text() -> Result<(), arrow_schema::ArrowError> {
        let batch = RecordBatch::try_from_iter([
            (
                "id",
                Arc::new(Int64Array::from(vec![Some(2), None])) as Arc<dyn Array>,
            ),
            ("detail", Arc::new(StringArray::from(vec!["SCAN t", "x"]))),
            (
                "raw",
                Arc::new(BinaryArray::from(vec![b"{}".as_slice(), b"[]".as_slice()])),
            ),
            ("flag", Arc::new(BooleanArray::from(vec![true, false]))),
        ])?;
        let sink = TextRows::default();
        assert_eq!(sink.push(ResultId::new(), batch), SinkFlow::Continue);
        let rows = sink.take();
        assert_eq!(
            rows[0],
            [
                Some("2".to_owned()),
                Some("SCAN t".to_owned()),
                Some("{}".to_owned()),
                Some("true".to_owned())
            ]
        );
        assert_eq!(rows[1][0], None);
        assert!(sink.take().is_empty());
        Ok(())
    }
    #[test]
    fn showplan_collector_discards_query_rows_and_bounds_xml()
    -> Result<(), arrow_schema::ArrowError> {
        let sink = TextRows::new(true);
        let result = ResultId::new();
        let data = RecordBatch::try_from_iter([(
            "ordinary",
            Arc::new(StringArray::from(vec!["<ShowPlanXML/>"; 200])) as Arc<dyn Array>,
        )])?;
        sink.begin(result, data.schema());
        assert_eq!(sink.push(result, data), SinkFlow::Continue);
        assert!(sink.take().is_empty());
        let plan = RecordBatch::try_from_iter([(
            "Microsoft SQL Server 2005 XML Showplan",
            Arc::new(StringArray::from(vec!["<ShowPlanXML/>"; 2])) as Arc<dyn Array>,
        )])?;
        sink.begin(result, plan.schema());
        sink.push(result, plan);
        assert_eq!(sink.take().len(), 2);
        let oversized = "x".repeat(8 * 1024 * 1024 + 1);
        let plan = RecordBatch::try_from_iter([(
            "Microsoft SQL Server 2005 XML Showplan",
            Arc::new(StringArray::from(vec![oversized])) as Arc<dyn Array>,
        )])?;
        assert_eq!(sink.push(result, plan), SinkFlow::Stop);
        assert!(sink.over_budget());
        Ok(())
    }
}
