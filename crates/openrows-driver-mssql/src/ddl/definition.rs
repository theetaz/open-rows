// SPDX-License-Identifier: Apache-2.0
//! The `CREATE TABLE` statement rebuilt from the catalog (SQL Server has no
//! `SHOW CREATE TABLE`), one column per line with its collation and named
//! default constraint, and reading those two back for the structure editor.
use std::fmt::Write as _;

use openrows_driver_api::schema::{ConstraintDetail, ConstraintKind};
use openrows_sql::Dialect;
use openrows_sql::ddl::TableDefinition;

use crate::catalog::rows::{ColumnRow, IndexRow};

/// A schema-qualified table.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Relation<'a> {
    pub(crate) schema: &'a str,
    pub(crate) table: &'a str,
}

impl Relation<'_> {
    /// `[schema].[table]`.
    pub(crate) fn quoted(&self) -> String {
        format!("{}.{}", quote(self.schema), quote(self.table))
    }
}

/// `[name]` with `]` doubled. Catalog names never hold NUL.
pub(crate) fn quote(name: &str) -> String {
    format!("[{}]", name.replace(']', "]]"))
}

/// One column's definition.
pub(crate) fn column_line(column: &ColumnRow) -> String {
    let mut line = quote(&column.name);
    if let Some(expression) = &column.computed {
        let _ = write!(line, " AS {expression}");
        return line;
    }
    let _ = write!(line, " {}", column.data_type);
    if let Some((seed, increment)) = &column.identity {
        let _ = write!(line, " IDENTITY({seed}, {increment})");
    }
    if let Some(collation) = &column.collation {
        let _ = write!(line, " COLLATE {collation}");
    }
    line.push_str(if column.nullable {
        " NULL"
    } else {
        " NOT NULL"
    });
    if let (Some(name), Some(default)) = (&column.default_name, &column.default) {
        let _ = write!(line, " CONSTRAINT {} DEFAULT {default}", quote(name));
    }
    line
}

/// `CREATE TABLE` with constraints, then one `CREATE INDEX` per index that
/// backs no constraint.
pub(crate) fn create_table(
    relation: &Relation<'_>,
    columns: &[ColumnRow],
    constraints: &[ConstraintDetail],
    indexes: &[IndexRow],
) -> String {
    let mut lines: Vec<String> = columns.iter().map(column_line).collect();
    for constraint in constraints {
        let body = match constraint.kind {
            ConstraintKind::Primary | ConstraintKind::Unique => key_constraint(constraint, indexes),
            _ => constraint.definition.clone(),
        };
        lines.push(format!("CONSTRAINT {} {body}", quote(&constraint.name)));
    }
    let mut sql = format!(
        "CREATE TABLE {} (\n    {}\n);\n",
        relation.quoted(),
        lines.join(",\n    ")
    );
    for index in indexes {
        if !(index.primary || index.unique_constraint) {
            let _ = writeln!(sql, "{};", index_definition(relation, index));
        }
    }
    sql
}

/// `PRIMARY KEY CLUSTERED ([a])` with the backing index's kind.
fn key_constraint(constraint: &ConstraintDetail, indexes: &[IndexRow]) -> String {
    let keyword = if constraint.kind == ConstraintKind::Primary {
        "PRIMARY KEY"
    } else {
        "UNIQUE"
    };
    let index = indexes.iter().find(|index| index.name == constraint.name);
    let method = index.map_or_else(String::new, |index| {
        format!(" {}", index.method.to_ascii_uppercase())
    });
    let columns = index.map_or_else(
        || {
            constraint
                .columns
                .iter()
                .map(|name| quote(name))
                .collect::<Vec<_>>()
                .join(", ")
        },
        quoted_keys,
    );
    format!("{keyword}{method} ({columns})")
}

fn quoted_keys(index: &IndexRow) -> String {
    index
        .columns
        .iter()
        .map(|(name, descending)| {
            let order = if *descending { " DESC" } else { "" };
            format!("{}{order}", quote(name))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// `CREATE [UNIQUE] <kind> INDEX [name] ON [s].[t] (…) [INCLUDE (…)] [WHERE …]`.
pub(crate) fn index_definition(relation: &Relation<'_>, index: &IndexRow) -> String {
    let unique = if index.unique { "UNIQUE " } else { "" };
    let mut sql = format!(
        "CREATE {unique}{} INDEX {} ON {} ({})",
        index.method.to_ascii_uppercase(),
        quote(&index.name),
        relation.quoted(),
        quoted_keys(index)
    );
    if !index.include.is_empty() {
        let include: Vec<String> = index.include.iter().map(|name| quote(name)).collect();
        let _ = write!(sql, " INCLUDE ({})", include.join(", "));
    }
    if let Some(filter) = &index.filter {
        let _ = write!(sql, " WHERE {filter}");
    }
    sql
}

/// What the structure editor needs from a column's definition line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ColumnClauses {
    /// Name of the column's default constraint.
    pub(crate) default_constraint: Option<String>,
    pub(crate) collation: Option<String>,
}

/// The clauses of `column` in a definition written by [`create_table`].
pub(crate) fn column_clauses(definition: &str, column: &str) -> Option<ColumnClauses> {
    let table = TableDefinition::parse(definition, Dialect::SqlServer)?;
    let element = table
        .elements
        .iter()
        .find(|element| element.column.as_deref() == Some(column))?;
    let text = element.text.as_str();
    Some(ColumnClauses {
        default_constraint: text
            .find(" CONSTRAINT [")
            .and_then(|at| bracketed(&text[at + " CONSTRAINT ".len()..])),
        collation: text.find(" COLLATE ").and_then(|at| {
            text[at + " COLLATE ".len()..]
                .split_whitespace()
                .next()
                .map(str::to_owned)
        }),
    })
}

/// The name of a leading `[…]` identifier, `]]` unescaped.
fn bracketed(text: &str) -> Option<String> {
    let mut name = String::new();
    let mut chars = text.strip_prefix('[')?.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ']' {
            if chars.peek() == Some(&']') {
                chars.next();
            } else {
                return Some(name);
            }
        }
        name.push(c);
    }
    None
}
