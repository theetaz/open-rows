// SPDX-License-Identifier: Apache-2.0
use super::*;
use arrow_array::Array;

fn spec(name: &str, declared: Option<&str>) -> ColumnSpec {
    ColumnSpec {
        name: name.to_owned(),
        declared: declared.map(str::to_owned),
    }
}

fn batch(specs: Vec<ColumnSpec>, rows: Vec<Vec<Cell>>) -> Result<RecordBatch, ArrowError> {
    let mut buffer = BatchBuffer::new(specs.into());
    for row in rows {
        buffer.push(row);
    }
    buffer.finish()
}

#[test]
fn affinity_follows_the_declared_type() {
    let cases = [
        (Some("INTEGER"), Affinity::Integer),
        (Some("bigint unsigned"), Affinity::Integer),
        (Some("VARCHAR(40)"), Affinity::Text),
        (Some("CLOB"), Affinity::Text),
        (Some("BLOB"), Affinity::Blob),
        (None, Affinity::Blob),
        (Some("DOUBLE PRECISION"), Affinity::Real),
        (Some("FLOAT"), Affinity::Real),
        (Some("DECIMAL(10,2)"), Affinity::Numeric),
        (Some("DATETIME"), Affinity::Numeric),
        (Some("BOOLEAN"), Affinity::Numeric),
    ];
    for (declared, expected) in cases {
        assert_eq!(Affinity::of(declared), expected, "{declared:?}");
    }
}

#[test]
fn uniform_columns_get_typed_arrays() -> Result<(), ArrowError> {
    let batch = batch(
        vec![
            spec("id", Some("INTEGER")),
            spec("price", Some("REAL")),
            spec("name", Some("TEXT")),
            spec("data", Some("BLOB")),
        ],
        vec![
            vec![
                Cell::Integer(1),
                Cell::Integer(2),
                Cell::Text("a".into()),
                Cell::Blob(vec![1]),
            ],
            vec![Cell::Integer(2), Cell::Real(2.5), Cell::Null, Cell::Null],
        ],
    )?;
    let types: Vec<_> = batch
        .schema()
        .fields()
        .iter()
        .map(|field| field.data_type().clone())
        .collect();
    assert_eq!(
        types,
        vec![
            DataType::Int64,
            DataType::Float64,
            DataType::Utf8,
            DataType::Binary
        ]
    );
    let prices = batch
        .column(1)
        .as_any()
        .downcast_ref::<Float64Array>()
        .map(|array| (array.value(0), array.value(1)));
    assert_eq!(prices, Some((2.0, 2.5)));
    assert!(batch.column(2).is_null(1));
    let source = batch.schema().field(1).metadata().get(SOURCE_TYPE).cloned();
    assert_eq!(source.as_deref(), Some("REAL"));
    Ok(())
}

#[test]
fn mixed_storage_classes_fall_back_to_exact_tagged_text() -> Result<(), ArrowError> {
    let batch = batch(
        vec![spec("v", Some("NUMERIC")), spec("e", None)],
        vec![
            vec![Cell::Integer(7), Cell::Real(0.5)],
            vec![Cell::Text("seven".into()), Cell::Null],
            vec![Cell::Blob(vec![0xab, 0x01]), Cell::Null],
        ],
    )?;
    let field = batch.schema().field(0).clone();
    assert_eq!(field.data_type(), &DataType::Utf8);
    assert_eq!(
        field.metadata().get(TEXT_FALLBACK).map(String::as_str),
        Some("true")
    );
    let values = batch
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .map(|array| {
            (0..3)
                .map(|i| array.value(i).to_owned())
                .collect::<Vec<_>>()
        });
    assert_eq!(
        values,
        Some(vec![
            "7".to_owned(),
            "seven".to_owned(),
            "x'ab01'".to_owned()
        ])
    );
    let expression = batch.schema().field(1).clone();
    assert_eq!(expression.data_type(), &DataType::Float64);
    assert_eq!(
        expression.metadata().get(SOURCE_TYPE).map(String::as_str),
        Some("REAL")
    );
    Ok(())
}

#[test]
fn integers_and_reals_mix_only_in_real_columns() -> Result<(), ArrowError> {
    let rows = || vec![vec![Cell::Integer(1)], vec![Cell::Real(1.5)]];
    let real = batch(vec![spec("r", Some("REAL"))], rows())?;
    assert_eq!(real.schema().field(0).data_type(), &DataType::Float64);
    let numeric = batch(vec![spec("n", Some("NUMERIC"))], rows())?;
    assert_eq!(numeric.schema().field(0).data_type(), &DataType::Utf8);
    let empty = batch(vec![spec("n", Some("INT"))], Vec::new())?;
    assert_eq!(empty.num_rows(), 0);
    assert_eq!(empty.schema().field(0).data_type(), &DataType::Int64);
    Ok(())
}
