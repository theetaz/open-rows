// SPDX-License-Identifier: Apache-2.0
use super::{TestResult, export_one};
use crate::{Format, JsonLayout};
use arrow_array::{Array, Int64Array, ListArray, RecordBatch, StructArray, types::Int64Type};
use arrow_schema::{DataType, Field, Schema};
use std::{collections::HashMap, sync::Arc};

fn field(name: &str, data_type: DataType) -> Field {
    Field::new(name, data_type, true).with_metadata(HashMap::from([(
        "openrows.source_type".into(),
        "EJSON".into(),
    )]))
}

fn documents() -> TestResult<RecordBatch> {
    let list = ListArray::from_iter_primitive::<Int64Type, _, _>([
        Some(vec![Some(9_007_199_254_740_993), None]),
        None,
        Some(vec![Some(2)]),
    ]);
    let item = Arc::new(field("item", DataType::Int64));
    let list = ListArray::try_new(
        item.clone(),
        list.offsets().clone(),
        list.values().clone(),
        list.nulls().cloned(),
    )?;
    let fields = vec![
        field("n", DataType::Int64),
        field("items", DataType::List(item)),
    ];
    let structure = StructArray::try_new(
        fields.into(),
        vec![
            Arc::new(Int64Array::from(vec![Some(1), None, Some(3)])),
            Arc::new(list),
        ],
        None,
    )?;
    let schema = Arc::new(Schema::new(vec![field(
        "nested",
        structure.data_type().clone(),
    )]));
    Ok(RecordBatch::try_new(schema, vec![Arc::new(structure)])?)
}

#[test]
fn document_exports_keep_nested_json_nulls_and_exact_integer_digits() -> TestResult {
    let batch = documents()?;
    assert_eq!(
        export_one(&Format::Json(JsonLayout::Lines), &batch)?,
        "{\"nested\":{\"n\":1,\"items\":[9007199254740993,null]}}\n{\"nested\":{\"n\":null,\"items\":null}}\n{\"nested\":{\"n\":3,\"items\":[2]}}\n"
    );
    assert_eq!(
        export_one(&Format::Json(JsonLayout::Lines), &batch.slice(1, 2))?,
        "{\"nested\":{\"n\":null,\"items\":null}}\n{\"nested\":{\"n\":3,\"items\":[2]}}\n"
    );
    let csv = export_one(&Format::Csv(crate::CsvOptions::default()), &batch)?;
    assert!(csv.contains("9007199254740993"));
    assert!(csv.contains("[2]"));
    Ok(())
}
