// SPDX-License-Identifier: Apache-2.0
//! Column definitions, numbering, collations, foreign keys, checks and
//! indexes as `MySQL` SQL, shared by `ALTER TABLE` and `CREATE TABLE`.
use super::{DIALECT, data_type, default_sql, literal, quote};
use openrows_driver_api::schema::{
    Identity, IndexMethod, NewCheck, NewColumn, NewForeignKey, NewIndex, ReferentialAction,
};
use openrows_driver_api::structure::AlterError;
use openrows_sql::ddl::ColumnDefinition;
use std::fmt::Write as _;

/// `AUTO_INCREMENT` on or off; identity columns are `PostgreSQL`'s.
pub(super) fn auto_increment(identity: Identity) -> Result<bool, AlterError> {
    match identity {
        Identity::None => Ok(false),
        Identity::AutoIncrement => Ok(true),
        Identity::Always | Identity::ByDefault => Err(AlterError::Unsupported(
            "MySQL numbers rows with AUTO_INCREMENT".to_owned(),
        )),
    }
}

/// A character set or collation name: letters, digits and `_` only.
pub(super) fn charset_name(name: Option<&str>) -> Result<Option<&str>, AlterError> {
    match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) if name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') => {
            Ok(Some(name))
        }
        Some(name) => Err(AlterError::InvalidType(name.to_owned())),
        None => Ok(None),
    }
}

/// `` `name` type [CHARACTER SET …] [COLLATE …] [NOT NULL] [DEFAULT …] [AUTO_INCREMENT] [COMMENT …] ``.
pub(super) fn column_definition(column: &NewColumn) -> Result<String, AlterError> {
    let mut definition =
        ColumnDefinition::new(&quote(&column.name)?, &data_type(&column.data_type)?);
    definition.set_collation(
        charset_name(column.charset.as_deref())?,
        charset_name(column.collation.as_deref())?,
    );
    definition.set_nullable(column.nullable);
    let default = default_sql(&column.name, &column.default)?;
    let numbered = auto_increment(column.identity)?;
    if numbered && default.is_some() {
        return Err(AlterError::Conflict(column.name.clone()));
    }
    definition.set_default(default.as_deref());
    definition.set_auto_increment(numbered);
    let comment = column.comment.as_deref().map(literal).transpose()?;
    definition.set_comment(comment.as_deref());
    Ok(definition.render())
}

/// `` `a`, `b` ``.
pub(super) fn column_list(columns: &[String]) -> Result<String, AlterError> {
    Ok(columns
        .iter()
        .map(|column| quote(column))
        .collect::<Result<Vec<_>, _>>()?
        .join(", "))
}

fn constraint_name(name: Option<&str>) -> Result<String, AlterError> {
    match name.filter(|name| !name.is_empty()) {
        Some(name) => Ok(format!("CONSTRAINT {} ", quote(name)?)),
        None => Ok(String::new()),
    }
}

/// `[CONSTRAINT n] FOREIGN KEY (…) REFERENCES db.t (…) [ON DELETE …] [ON UPDATE …]`.
pub(super) fn foreign_key(key: &NewForeignKey, schema: &str) -> Result<String, AlterError> {
    let target_schema = Some(key.target.schema.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(schema);
    let target = DIALECT
        .qualified(target_schema, &key.target.table)
        .map_err(|_| AlterError::InvalidName)?;
    let mut sql = format!(
        "{}FOREIGN KEY ({}) REFERENCES {target} ({})",
        constraint_name(key.name.as_deref())?,
        column_list(&key.columns)?,
        column_list(&key.target.columns)?
    );
    for (event, action) in [("DELETE", key.on_delete), ("UPDATE", key.on_update)] {
        if action != ReferentialAction::NoAction {
            let _ = write!(sql, " ON {event} {}", action.sql());
        }
    }
    Ok(sql)
}

/// `[CONSTRAINT n] CHECK (expression)`.
pub(super) fn check(check: &NewCheck) -> Result<String, AlterError> {
    Ok(format!(
        "{}CHECK ({})",
        constraint_name(check.name.as_deref())?,
        check.expression.trim()
    ))
}

/// `[UNIQUE] INDEX [name] (keys) USING method`, as `ADD …` or a table element.
pub(super) fn index_spec(index: &NewIndex) -> Result<String, AlterError> {
    let method = match index.method {
        IndexMethod::Btree => "BTREE",
        IndexMethod::Hash => "HASH",
        other => {
            return Err(AlterError::Unsupported(format!(
                "MySQL has no {other:?} indexes; use B-tree or hash"
            )));
        }
    };
    if index.predicate.is_some() {
        return Err(AlterError::Unsupported(
            "MySQL has no partial indexes".to_owned(),
        ));
    }
    let unique = if index.unique { "UNIQUE " } else { "" };
    let name = match index.name.as_deref().filter(|name| !name.is_empty()) {
        Some(name) => format!("{} ", quote(name)?),
        None => String::new(),
    };
    let keys = index
        .columns
        .iter()
        .map(|column| {
            let order = if index.descending.contains(column) {
                " DESC"
            } else {
                ""
            };
            quote(column).map(|quoted| format!("{quoted}{order}"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    Ok(format!("{unique}INDEX {name}({keys}) USING {method}"))
}
