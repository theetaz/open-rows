// SPDX-License-Identifier: Apache-2.0
//! Nested document columns stay JSON across the shared export formats.
use crate::cell::Column;
use crate::json::{push_string, push_value};
use arrow_array::{Array, ListArray, cast::AsArray};
use arrow_schema::{DataType, Field};

pub(crate) enum Document<'a> {
    Object(Vec<(String, Column<'a>)>),
    List(&'a ListArray, Box<Column<'a>>),
}

impl<'a> Document<'a> {
    pub(crate) fn new(field: &Field, array: &'a dyn Array) -> Option<Self> {
        match field.data_type() {
            DataType::Struct(fields) => {
                let structure = array.as_struct_opt()?;
                let columns = fields
                    .iter()
                    .zip(structure.columns())
                    .map(|(field, array)| {
                        Column::new(field, array.as_ref())
                            .ok()
                            .map(|column| (field.name().clone(), column))
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(Self::Object(columns))
            }
            DataType::List(item) => {
                let list = array.as_list_opt::<i32>()?;
                let column = Column::new(item, list.values().as_ref()).ok()?;
                Some(Self::List(list, Box::new(column)))
            }
            _ => None,
        }
    }

    pub(crate) fn json(&self, row: usize) -> String {
        match self {
            Self::Object(columns) => object_json(columns, row),
            Self::List(list, column) => list_json(list, column, row),
        }
    }
}

fn object_json(columns: &[(String, Column<'_>)], row: usize) -> String {
    let mut out = String::from("{");
    let mut scratch = String::new();
    for (index, (name, column)) in columns.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        push_string(name, &mut out);
        out.push(':');
        push_value(&column.cell(row), &mut out, &mut scratch);
    }
    out.push('}');
    out
}

fn list_json(list: &ListArray, column: &Column<'_>, row: usize) -> String {
    let mut out = String::from("[");
    let mut scratch = String::new();
    let offsets = list.value_offsets();
    let start = usize::try_from(offsets[row]).unwrap_or_default();
    let end = usize::try_from(offsets[row + 1]).unwrap_or(start);
    for (index, item) in (start..end).enumerate() {
        if index > 0 {
            out.push(',');
        }
        push_value(&column.cell(item), &mut out, &mut scratch);
    }
    out.push(']');
    out
}
