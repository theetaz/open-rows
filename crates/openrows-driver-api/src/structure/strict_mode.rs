// SPDX-License-Identifier: Apache-2.0
//! Execution precondition: `MariaDB` can coerce NULLs even with ALGORITHM=INPLACE.
use crate::{Connection, DriverError, ResultSink, SinkFlow};
use arrow_array::{Array, LargeStringArray, RecordBatch, StringArray, StringViewArray};
use arrow_schema::SchemaRef;

pub(super) async fn require(connection: &mut dyn Connection) -> Result<(), DriverError> {
    let mut mode = Mode::default();
    connection
        .execute("SELECT @@SESSION.sql_mode", &mut mode)
        .await?;
    if mode.strict {
        Ok(())
    } else {
        Err(DriverError::Unsupported(
            "Adding a primary key requires STRICT_TRANS_TABLES or STRICT_ALL_TABLES in this session. Enable strict mode and review again; no DDL was run.".into(),
        ))
    }
}

fn strict(mode: &str) -> bool {
    mode.split(',').map(str::trim).any(|name| {
        name.eq_ignore_ascii_case("STRICT_TRANS_TABLES")
            || name.eq_ignore_ascii_case("STRICT_ALL_TABLES")
    })
}

#[derive(Default)]
struct Mode {
    strict: bool,
}

impl ResultSink for Mode {
    fn begin_result(&mut self, _schema: SchemaRef) {
        self.strict = false;
    }

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.strict = false;
        if batch.num_rows() != 1 || batch.num_columns() != 1 {
            return SinkFlow::Continue;
        }
        let Some(column) = batch.columns().first() else {
            return SinkFlow::Continue;
        };
        if column.is_empty() || column.is_null(0) {
            return SinkFlow::Continue;
        }
        let any = column.as_any();
        let value = any
            .downcast_ref::<StringArray>()
            .map(|array| array.value(0))
            .or_else(|| {
                any.downcast_ref::<LargeStringArray>()
                    .map(|array| array.value(0))
            })
            .or_else(|| {
                any.downcast_ref::<StringViewArray>()
                    .map(|array| array.value(0))
            });
        self.strict = value.is_some_and(strict);
        SinkFlow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::Int32Array;
    use std::sync::Arc;

    #[test]
    fn strict_mode_requires_a_complete_flag() {
        for mode in [
            "STRICT_TRANS_TABLES",
            "ANSI_QUOTES, strict_all_tables ,NO_ZERO_DATE",
        ] {
            assert!(strict(mode));
        }
        for mode in [
            "",
            "ANSI_QUOTES",
            "NO_STRICT_TRANS_TABLES",
            "STRICT_ALL_TABLES_EXTRA",
        ] {
            assert!(!strict(mode));
        }
    }

    #[test]
    fn null_or_nontext_results_fail_closed() -> Result<(), arrow_schema::ArrowError> {
        let mut mode = Mode::default();
        for values in [
            vec![None],
            vec![Some("STRICT_TRANS_TABLES")],
            vec![Some("")],
        ] {
            let array = Arc::new(StringArray::from(values));
            let batch = RecordBatch::try_from_iter([("sql_mode", array as Arc<dyn Array>)])?;
            mode.push_batch(batch);
        }
        assert!(!mode.strict);
        let batch = RecordBatch::try_from_iter([(
            "sql_mode",
            Arc::new(Int32Array::from(vec![1])) as Arc<dyn Array>,
        )])?;
        mode.push_batch(batch);
        assert!(!mode.strict);
        Ok(())
    }
}
