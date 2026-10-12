// SPDX-License-Identifier: Apache-2.0
//! Shared document result mapping for document and key/value engines.
use arrow_array::{
    ArrayRef, BooleanArray, Float64Array, Int64Array, ListArray, RecordBatch, StringArray,
    StructArray, new_null_array,
};
use arrow_buffer::{NullBuffer, OffsetBuffer, ScalarBuffer};
use arrow_schema::{ArrowError, DataType, Field, Fields, Schema, SchemaRef};
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

/// Full lossless document column; typed fields are a sampled table projection.
pub const DOCUMENT_COLUMN: &str = "__document";

/// Infer one stable result schema from a bounded sample, preserving JSON/EJSON.
#[must_use]
pub fn schema(sample: &[Value]) -> SchemaRef {
    let names: BTreeSet<_> = sample
        .iter()
        .filter_map(Value::as_object)
        .flat_map(serde_json::Map::keys)
        .filter(|name| name.as_str() != DOCUMENT_COLUMN)
        .collect();
    let mut fields: Vec<_> = names
        .into_iter()
        .map(|name| {
            let values: Vec<_> = sample.iter().map(|row| &row[name]).collect();
            let ty = if name == "_id" {
                DataType::Utf8
            } else {
                infer(&values, 0)
            };
            field(name, ty)
        })
        .collect();
    fields.push(field(DOCUMENT_COLUMN, DataType::Utf8));
    let metadata = HashMap::from([("openrows.result_kind".into(), "documents".into())]);
    Arc::new(Schema::new_with_metadata(fields, metadata))
}

fn field(name: &str, ty: DataType) -> Field {
    let mut field = Field::new(name, ty, true).with_metadata(HashMap::from([(
        "openrows.source_type".into(),
        "EJSON".into(),
    )]));
    if field.data_type() == &DataType::Utf8 {
        field = field.with_metadata(HashMap::from([
            ("ARROW:extension:name".into(), "openrows.json".into()),
            ("openrows.source_type".into(), "EJSON".into()),
        ]));
    }
    field
}

fn infer(values: &[&Value], depth: usize) -> DataType {
    if depth >= 16 {
        return DataType::Utf8;
    }
    let present: Vec<_> = values
        .iter()
        .copied()
        .filter(|value| !value.is_null())
        .collect();
    let Some(first) = present.first() else {
        return DataType::Utf8;
    };
    if present.iter().all(|v| integer(v).is_some()) {
        return DataType::Int64;
    }
    if present.iter().all(|v| number(v).is_some()) {
        return DataType::Float64;
    }
    match first {
        Value::Bool(_) if present.iter().all(|v| v.is_boolean()) => DataType::Boolean,
        Value::Object(_) if present.iter().all(|v| v.is_object()) => object_type(&present, depth),
        Value::Array(_) if present.iter().all(|v| v.is_array()) => {
            let elements: Vec<_> = present
                .iter()
                .filter_map(|v| v.as_array())
                .flatten()
                .collect();
            DataType::List(Arc::new(field("item", infer(&elements, depth + 1))))
        }
        _ => DataType::Utf8,
    }
}

fn object_type(values: &[&Value], depth: usize) -> DataType {
    let names: BTreeSet<_> = values
        .iter()
        .filter_map(|v| v.as_object())
        .flat_map(serde_json::Map::keys)
        .collect();
    let fields: Vec<_> = names
        .into_iter()
        .map(|name| {
            let children: Vec<_> = values.iter().map(|v| &v[name]).collect();
            field(name, infer(&children, depth + 1))
        })
        .collect();
    if fields.is_empty() {
        DataType::Utf8
    } else {
        DataType::Struct(fields.into())
    }
}

/// Map a bounded batch using the result's stable schema. New or changed shapes
/// stay lossless in `__document`, even when a sampled typed projection is null.
/// # Errors
/// Arrow array/schema inconsistencies or excessive list offsets.
pub fn batch(schema: SchemaRef, rows: &[Value]) -> Result<RecordBatch, ArrowError> {
    let columns = schema
        .fields()
        .iter()
        .map(|field| {
            let values: Vec<_> = rows
                .iter()
                .map(|row| {
                    if field.name() == DOCUMENT_COLUMN {
                        row
                    } else {
                        &row[field.name()]
                    }
                })
                .collect();
            array(field.data_type(), &values)
        })
        .collect::<Result<Vec<_>, _>>()?;
    RecordBatch::try_new(schema, columns)
}

fn array(ty: &DataType, values: &[&Value]) -> Result<ArrayRef, ArrowError> {
    let array: ArrayRef = match ty {
        DataType::Boolean => Arc::new(BooleanArray::from(
            values.iter().map(|v| v.as_bool()).collect::<Vec<_>>(),
        )),
        DataType::Int64 => Arc::new(Int64Array::from(
            values.iter().map(|v| integer(v)).collect::<Vec<_>>(),
        )),
        DataType::Float64 => Arc::new(Float64Array::from(
            values.iter().map(|v| number(v)).collect::<Vec<_>>(),
        )),
        DataType::Struct(fields) => structure(fields, values)?,
        DataType::List(field) => list(field, values)?,
        DataType::Utf8 => Arc::new(StringArray::from(
            values
                .iter()
                .map(|v| (!v.is_null()).then(|| v.to_string()))
                .collect::<Vec<_>>(),
        )),
        _ => new_null_array(ty, values.len()),
    };
    Ok(array)
}

fn structure(fields: &Fields, values: &[&Value]) -> Result<ArrayRef, ArrowError> {
    let columns = fields
        .iter()
        .map(|field| {
            let children: Vec<_> = values.iter().map(|v| &v[field.name()]).collect();
            array(field.data_type(), &children)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let nulls = NullBuffer::from(values.iter().map(|v| v.is_object()).collect::<Vec<_>>());
    Ok(Arc::new(StructArray::try_new(
        fields.clone(),
        columns,
        Some(nulls),
    )?))
}

fn list(field: &Arc<Field>, values: &[&Value]) -> Result<ArrayRef, ArrowError> {
    let mut offsets = vec![0_i32];
    let mut children = Vec::new();
    for value in values {
        if let Some(items) = value.as_array() {
            children.extend(items.iter());
        }
        offsets.push(i32::try_from(children.len()).map_err(|_| {
            ArrowError::InvalidArgumentError("document list exceeds Arrow offsets".into())
        })?);
    }
    let array = array(field.data_type(), &children)?;
    let nulls = NullBuffer::from(values.iter().map(|v| v.is_array()).collect::<Vec<_>>());
    Ok(Arc::new(ListArray::try_new(
        field.clone(),
        OffsetBuffer::new(ScalarBuffer::from(offsets)),
        array,
        Some(nulls),
    )?))
}

fn integer(value: &Value) -> Option<i64> {
    if value.as_object().is_some_and(|object| object.len() != 1) {
        return None;
    }
    value.as_i64().or_else(|| {
        value
            .get("$numberInt")
            .or_else(|| value.get("$numberLong"))?
            .as_str()?
            .parse()
            .ok()
    })
}
fn number(value: &Value) -> Option<f64> {
    if let Some(integer) = value.as_i64() {
        return value
            .as_f64()
            .filter(|_| (-9_007_199_254_740_992..=9_007_199_254_740_992).contains(&integer));
    }
    if value.is_u64() && !value.is_i64()
        || value.as_object().is_some_and(|object| object.len() != 1)
    {
        return None;
    }
    value
        .as_f64()
        .or_else(|| value.get("$numberDouble")?.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn mixed_large_json_integers_use_exact_json_instead_of_rounded_floats() -> Result<(), ArrowError>
    {
        let rows = [json!({"n":9_007_199_254_740_993_i64}), json!({"n":1.5})];
        let schema = schema(&rows);
        assert_eq!(schema.field(0).data_type(), &DataType::Utf8);
        let mapped = batch(schema, &rows)?;
        let values = mapped.column(0).as_any().downcast_ref::<StringArray>();
        assert!(values.is_some_and(|values| values.value(0) == "9007199254740993"));
        Ok(())
    }
    #[test]
    fn nested_fields_lists_and_heterogeneous_values_keep_lossless_json() -> Result<(), ArrowError> {
        let rows = vec![
            json!({"nested":{"n":1},"list":[1,2],"mixed":false}),
            json!({"nested":{"n":2},"list":[],"mixed":{"x":1}}),
        ];
        let schema = schema(&rows);
        assert!(matches!(schema.field(2).data_type(), DataType::Struct(_)));
        let mapped = batch(schema.clone(), &rows)?;
        assert_eq!(mapped.num_rows(), 2);
        let changed = batch(schema, &[json!({"nested":"changed","new":1})])?;
        let json = changed.column(3).as_any().downcast_ref::<StringArray>();
        assert!(json.is_some_and(|a| a.value(0).contains("new")));
        Ok(())
    }
}
