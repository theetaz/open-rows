// SPDX-License-Identifier: Apache-2.0
//! T-SQL pieces shared by structure changes and object lifecycle plans:
//! checked names, types, literals and defaults, column definitions, index,
//! foreign-key and check clauses, and `MS_Description` extended properties.
use std::fmt::Write as _;

use openrows_driver_api::schema::{
    DefaultValue, Identity, IndexMethod, NewCheck, NewColumn, NewForeignKey, NewIndex,
    ReferentialAction,
};
use openrows_driver_api::structure::AlterError;
use openrows_sql::Dialect;
use openrows_sql::ddl::{string_literal, valid_type};

use super::definition::{Relation, quote};

const DIALECT: Dialect = Dialect::SqlServer;

/// A quoted name; empty names and NUL are refused.
pub(crate) fn checked(name: &str) -> Result<String, AlterError> {
    if name.is_empty() || name.contains('\0') {
        return Err(AlterError::InvalidName);
    }
    Ok(quote(name))
}

/// `[a], [b]`.
pub(crate) fn column_list(names: &[String]) -> Result<String, AlterError> {
    Ok(names
        .iter()
        .map(|name| checked(name))
        .collect::<Result<Vec<_>, _>>()?
        .join(", "))
}

pub(crate) fn literal(text: &str) -> Result<String, AlterError> {
    string_literal(DIALECT, text).ok_or(AlterError::InvalidName)
}

pub(crate) fn data_type(text: &str) -> Result<String, AlterError> {
    let trimmed = text.trim();
    if valid_type(DIALECT, trimmed) {
        Ok(trimmed.to_owned())
    } else {
        Err(AlterError::InvalidType(text.to_owned()))
    }
}

/// A collation name: letters, digits and `_` only (`Latin1_General_100_CI_AS`).
pub(crate) fn collation(name: &str) -> Result<String, AlterError> {
    let valid = !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if valid {
        Ok(name.to_owned())
    } else {
        Err(AlterError::Unsupported(format!(
            "{name:?} is not a collation name"
        )))
    }
}

/// SQL Server has collations but no per-column character sets.
pub(crate) fn no_charset(charset: Option<&str>) -> Result<(), AlterError> {
    match charset {
        Some(_) => Err(AlterError::Unsupported(
            "SQL Server columns have no character set; choose a collation (a _UTF8 one for UTF-8)"
                .to_owned(),
        )),
        None => Ok(()),
    }
}

/// A SQL expression written by the user (defaults, checks, predicates):
/// one expression, never a second statement.
pub(crate) fn expression(sql: &str) -> Result<String, AlterError> {
    let sql = sql.trim();
    if sql.is_empty() || sql.contains('\0') || sql.contains(';') {
        return Err(AlterError::InvalidExpression(sql.to_owned()));
    }
    Ok(sql.to_owned())
}

/// The SQL after `DEFAULT`, parenthesised; `None` for no default.
pub(crate) fn default_sql(
    column: &str,
    default: &DefaultValue,
) -> Result<Option<String>, AlterError> {
    match default {
        DefaultValue::None => Ok(None),
        DefaultValue::Literal(value) => literal(value).map(|text| Some(format!("({text})"))),
        DefaultValue::Expression(sql) => expression(sql)
            .map(|sql| Some(format!("({sql})")))
            .map_err(|_| AlterError::InvalidDefault(column.to_owned())),
    }
}

/// `[name] type [COLLATE c] [IDENTITY(1, 1)] NULL|NOT NULL [CONSTRAINT [DF_t_c]
/// DEFAULT x]`; `with_values` fills existing rows with the default.
pub(crate) fn column_spec(
    table: &str,
    column: &NewColumn,
    with_values: bool,
) -> Result<String, AlterError> {
    no_charset(column.charset.as_deref())?;
    let mut spec = format!(
        "{} {}",
        checked(&column.name)?,
        data_type(&column.data_type)?
    );
    if let Some(name) = &column.collation {
        let _ = write!(spec, " COLLATE {}", collation(name)?);
    }
    let default = default_sql(&column.name, &column.default)?;
    if column.identity != Identity::None {
        if default.is_some() {
            return Err(AlterError::InvalidDefault(column.name.clone()));
        }
        spec.push_str(" IDENTITY(1, 1)");
    }
    spec.push_str(if column.nullable {
        " NULL"
    } else {
        " NOT NULL"
    });
    if let Some(sql) = default {
        let constraint = checked(&format!("DF_{table}_{}", column.name))?;
        let values = if with_values { " WITH VALUES" } else { "" };
        let _ = write!(spec, " CONSTRAINT {constraint} DEFAULT {sql}{values}");
    }
    Ok(spec)
}

/// `CREATE [UNIQUE] INDEX [name] ON [s].[t] ([a], [b] DESC) [WHERE …]`.
pub(crate) fn index_statement(
    relation: &Relation<'_>,
    index: &NewIndex,
) -> Result<String, AlterError> {
    if index.method != IndexMethod::Btree {
        return Err(AlterError::Unsupported(format!(
            "SQL Server has no {:?} indexes; use B-tree",
            index.method
        )));
    }
    let columns = index
        .columns
        .iter()
        .map(|column| {
            let order = if index.descending.contains(column) {
                " DESC"
            } else {
                ""
            };
            checked(column).map(|quoted| format!("{quoted}{order}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let name = match index.name.as_deref().filter(|name| !name.is_empty()) {
        Some(name) => name.to_owned(),
        None => format!("IX_{}_{}", relation.table, index.columns.join("_")),
    };
    let unique = if index.unique { "UNIQUE " } else { "" };
    let mut sql = format!(
        "CREATE {unique}INDEX {} ON {} ({})",
        checked(&name)?,
        relation.quoted(),
        columns.join(", ")
    );
    if let Some(predicate) = &index.predicate {
        let _ = write!(sql, " WHERE {}", expression(predicate)?);
    }
    Ok(sql)
}

fn constraint_prefix(name: Option<&str>) -> Result<String, AlterError> {
    match name.filter(|name| !name.is_empty()) {
        Some(name) => Ok(format!("CONSTRAINT {} ", checked(name)?)),
        None => Ok(String::new()),
    }
}

/// T-SQL has no `RESTRICT`; `NO ACTION` refuses the change the same way.
fn action(action: ReferentialAction) -> &'static str {
    match action {
        ReferentialAction::Restrict => ReferentialAction::NoAction.sql(),
        other => other.sql(),
    }
}

/// `[CONSTRAINT [n]] FOREIGN KEY (…) REFERENCES [s].[t] (…) ON DELETE … ON UPDATE …`.
pub(crate) fn foreign_key_spec(key: &NewForeignKey) -> Result<String, AlterError> {
    let target = Relation {
        schema: &key.target.schema,
        table: &key.target.table,
    };
    checked(target.schema)?;
    checked(target.table)?;
    Ok(format!(
        "{}FOREIGN KEY ({}) REFERENCES {} ({}) ON DELETE {} ON UPDATE {}",
        constraint_prefix(key.name.as_deref())?,
        column_list(&key.columns)?,
        target.quoted(),
        column_list(&key.target.columns)?,
        action(key.on_delete),
        action(key.on_update)
    ))
}

/// `[CONSTRAINT [n]] CHECK (…)`.
pub(crate) fn check_spec(check: &NewCheck) -> Result<String, AlterError> {
    Ok(format!(
        "{}CHECK ({})",
        constraint_prefix(check.name.as_deref())?,
        expression(&check.expression)?
    ))
}

/// What an extended property describes.
#[derive(Clone, Copy)]
pub(crate) enum Level<'a> {
    Table,
    Column(&'a str),
}

/// Set, replace or remove an `MS_Description`; `exists` says whether one
/// is set now. `None` when nothing changes.
pub(crate) fn comment_statement(
    relation: &Relation<'_>,
    (level, exists): (Level<'_>, bool),
    comment: Option<&str>,
) -> Result<Option<String>, AlterError> {
    let mut arguments = vec!["@name = N'MS_Description'".to_owned()];
    let procedure = match (comment, exists) {
        (Some(text), _) => {
            arguments.push(format!("@value = {}", literal(text)?));
            if exists {
                "sp_updateextendedproperty"
            } else {
                "sp_addextendedproperty"
            }
        }
        (None, true) => "sp_dropextendedproperty",
        (None, false) => return Ok(None),
    };
    arguments.push(format!(
        "@level0type = N'SCHEMA', @level0name = {}",
        literal(relation.schema)?
    ));
    arguments.push(format!(
        "@level1type = N'TABLE', @level1name = {}",
        literal(relation.table)?
    ));
    if let Level::Column(column) = level {
        arguments.push(format!(
            "@level2type = N'COLUMN', @level2name = {}",
            literal(column)?
        ));
    }
    Ok(Some(format!(
        "EXEC sys.{procedure} {}",
        arguments.join(", ")
    )))
}
