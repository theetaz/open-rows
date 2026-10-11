// SPDX-License-Identifier: Apache-2.0
//! Bounded text collection without allocating full large cell strings.
use arrow_array::RecordBatch;
use arrow_cast::display::{ArrayFormatter, FormatOptions};
use arrow_schema::SchemaRef;
use openrows_driver_api::{ResultSink, SinkFlow};
use std::fmt::{self, Write};

pub(crate) struct Collector {
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<Option<String>>>,
    bytes: usize,
    cap: usize,
    field: usize,
    pub(crate) truncated: bool,
}
impl Collector {
    pub(crate) fn new(plan: bool) -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            bytes: 0,
            cap: if plan { 200 } else { 20 },
            field: if plan { 65536 } else { 512 },
            truncated: false,
        }
    }
    fn cell(&mut self, array: &dyn arrow_array::Array, row: usize) -> Option<String> {
        if array.is_null(row) {
            return None;
        }
        let mut text = Clipped {
            text: String::new(),
            cap: self.field.min(65536 - self.bytes),
        };
        if let Ok(formatter) = ArrayFormatter::try_new(array, &FormatOptions::default()) {
            self.truncated |= write!(text, "{}", formatter.value(row)).is_err();
        }
        self.bytes += text.text.len();
        Some(text.text)
    }
}
impl ResultSink for Collector {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.columns = schema
            .fields()
            .iter()
            .take(200)
            .map(|field| {
                if field.name().len() > 512 {
                    // Oversize provenance must redact its values, never lose a sensitive suffix.
                    "secret".into()
                } else {
                    field.name().clone()
                }
            })
            .collect();
    }
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        for row in 0..batch.num_rows() {
            if self.rows.len() >= self.cap
                || self.bytes >= 65536
                || (self.rows.len() + 1) * batch.num_columns() > 200
            {
                self.truncated = true;
                return SinkFlow::Stop;
            }
            let values: Vec<_> = batch
                .columns()
                .iter()
                .take(200)
                .map(|array| self.cell(array.as_ref(), row))
                .collect();
            self.rows.push(values);
        }
        SinkFlow::Continue
    }
}
struct Clipped {
    text: String,
    cap: usize,
}
impl Write for Clipped {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let mut end = text.len().min(self.cap.saturating_sub(self.text.len()));
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&text[..end]);
        if end < text.len() {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn column_provenance_is_never_truncated_before_redaction() {
        let within = format!("{}password", "x".repeat(128));
        let oversized = format!("{}password", "x".repeat(512));
        let mut rows = Collector::new(false);
        rows.begin_result(std::sync::Arc::new(arrow_schema::Schema::new(vec![
            arrow_schema::Field::new(&within, arrow_schema::DataType::Utf8, true),
            arrow_schema::Field::new(&oversized, arrow_schema::DataType::Utf8, true),
        ])));
        assert_eq!(rows.columns, vec![within, "secret".into()]);
    }
}
