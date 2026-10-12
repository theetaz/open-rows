// SPDX-License-Identifier: Apache-2.0
//! Document change-set plans retain values as EJSON and guard the whole original.
use crate::edit::{CellValue, Edit, EditKind, EditTable};
use crate::select::{BoundSql, SelectError};
use serde_json::{Value, json};

pub(crate) fn generate(table: &EditTable, edit: &Edit) -> Result<BoundSql, SelectError> {
    let collection = json!(table.table.name);
    let sql = match edit.kind {
        EditKind::Insert => format!(
            "db.getCollection({collection}).insertOne({})",
            replacement(edit)?
        ),
        EditKind::Update => {
            let value = replacement(edit)?;
            let original = original(edit)?;
            let mut value = value.as_object().cloned().ok_or_else(invalid)?;
            if !value.contains_key("_id") {
                value.insert(
                    "_id".into(),
                    original.get("_id").cloned().ok_or_else(invalid)?,
                );
            }
            format!(
                "db.getCollection({collection}).replaceOne({},{})",
                guard(edit)?,
                Value::Object(value)
            )
        }
        EditKind::Delete => format!("db.getCollection({collection}).deleteOne({})", guard(edit)?),
    };
    Ok(BoundSql {
        sql,
        params: Vec::new(),
    })
}
fn original(edit: &Edit) -> Result<Value, SelectError> {
    let text = edit
        .original
        .get("__document")
        .and_then(|v| v.as_deref())
        .ok_or_else(invalid)?;
    parse(text)
}
fn guard(edit: &Edit) -> Result<Value, SelectError> {
    let original = original(edit)?;
    let id = original.get("_id").ok_or_else(invalid)?;
    let key = edit
        .original
        .get("_id")
        .and_then(|v| v.as_deref())
        .ok_or_else(invalid)?;
    if serde_json::from_str::<Value>(key).map_err(|_| invalid())? != *id {
        return Err(invalid());
    }
    Ok(json!({"_id":id,"$expr":{"$eq":["$$ROOT",{"$literal":original}]}}))
}
fn replacement(edit: &Edit) -> Result<Value, SelectError> {
    let Some(CellValue::Text(text)) = edit.values.get("__document") else {
        return Err(invalid());
    };
    let value = parse(text)?;
    if edit.kind == EditKind::Insert && !edit.original.is_empty() {
        return Err(invalid());
    }
    if edit.values.keys().any(|key| key != "__document") {
        return Err(invalid());
    }
    Ok(value)
}
fn parse(text: &str) -> Result<Value, SelectError> {
    let value = super::value::document(text).map_err(|_| invalid())?;
    if !value.is_object() {
        return Err(invalid());
    }
    Ok(value)
}
fn invalid() -> SelectError {
    SelectError::Unsupported(
        "MongoDB edits require a valid JSON/EJSON object and the complete original _id/document snapshot",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn whole_document_guard_and_identity_are_required() -> Result<(), SelectError> {
        let table = EditTable {
            table: crate::select::TableRef {
                schema: "d".into(),
                name: "c".into(),
            },
            columns: vec![],
            key: vec![],
            protected: vec![],
        };
        let mut edit = Edit {
            kind: EditKind::Update,
            original: BTreeMap::from([
                ("_id".into(), Some("1".into())),
                ("__document".into(), Some("{\"_id\":1,\"a\":2}".into())),
            ]),
            values: BTreeMap::from([("__document".into(), CellValue::Text("{\"a\":3}".into()))]),
        };
        let plan = generate(&table, &edit)?;
        assert!(plan.sql.contains("$literal"));
        assert!(plan.sql.contains("replaceOne"));
        edit.values
            .insert("__document".into(), CellValue::Text("{\"_id\":2}".into()));
        assert!(generate(&table, &edit)?.sql.contains("\"_id\":2"));
        edit.kind = EditKind::Delete;
        edit.original.remove("_id");
        assert!(generate(&table, &edit).is_err());
        Ok(())
    }
}
