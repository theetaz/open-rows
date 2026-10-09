// SPDX-License-Identifier: Apache-2.0
//! Table detail: columns, key, unique and foreign-key constraints, indexes,
//! triggers, sizes and the server's own `SHOW CREATE` definition.
use openrows_driver_api::DriverError;
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, ForeignTarget, Generated, IndexDetail,
    RelationKind, TableDetail, TableInfo, TriggerDetail, TriggerEvent, TriggerTiming,
};
use openrows_sql::Dialect;

use super::{ColumnRow, IndexRow, bind, optional, text};
use crate::session::MySqlSession;

const TABLE_SQL: &str = "SELECT TABLE_TYPE, TABLE_ROWS, DATA_LENGTH, INDEX_LENGTH, TABLE_COMMENT \
    FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?";
const FOREIGN_SQL: &str = "SELECT CONSTRAINT_NAME, COLUMN_NAME, REFERENCED_TABLE_SCHEMA, \
    REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME FROM information_schema.KEY_COLUMN_USAGE \
    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND REFERENCED_TABLE_NAME IS NOT NULL \
    ORDER BY CONSTRAINT_NAME, ORDINAL_POSITION";
const TRIGGERS_SQL: &str = "SELECT TRIGGER_NAME, ACTION_TIMING, EVENT_MANIPULATION, ACTION_STATEMENT \
    FROM information_schema.TRIGGERS WHERE EVENT_OBJECT_SCHEMA = ? AND EVENT_OBJECT_TABLE = ? \
    ORDER BY TRIGGER_NAME";

/// Everything the Structure, Index, Triggers and DDL views show.
pub(crate) async fn table_detail(
    session: &mut MySqlSession,
    schema: &str,
    table: &str,
) -> Result<TableDetail, DriverError> {
    let found = session
        .text_query(TABLE_SQL, &bind(&[schema, table]))
        .await?;
    let Some(row) = found.first() else {
        return Ok(TableDetail::default());
    };
    let view = text(row, 0).contains("VIEW");
    let number = |index: usize| optional(row, index).and_then(|value| value.parse::<u64>().ok());
    let definition = show_create(session, schema, table).await?;
    let info = TableInfo {
        kind: if view {
            RelationKind::View
        } else {
            RelationKind::Table
        },
        row_estimate: number(1),
        data_bytes: number(2).unwrap_or_default(),
        index_bytes: number(3).unwrap_or_default(),
        total_bytes: number(2).unwrap_or_default() + number(3).unwrap_or_default(),
        comment: optional(row, 4),
        view_definition: view.then(|| definition.clone()),
        partition_key: None,
        definition: Some(definition),
    };
    let columns = session.column_rows(schema, table).await?;
    let indexes = session.index_rows(schema, table).await?;
    let mut constraints = key_constraints(&indexes);
    constraints.extend(foreign_keys(session, schema, table).await?);
    Ok(TableDetail {
        columns: columns.iter().map(column_detail).collect(),
        constraints,
        indexes: indexes
            .iter()
            .map(|index| index_detail(index, table))
            .collect(),
        triggers: triggers(session, schema, table).await?,
        partitions: Vec::new(),
        info: Some(info),
    })
}

/// `SHOW CREATE TABLE` (also prints views).
async fn show_create(
    session: &mut MySqlSession,
    schema: &str,
    table: &str,
) -> Result<String, DriverError> {
    let relation = Dialect::MySql
        .qualified(schema, table)
        .map_err(|error| DriverError::Unsupported(error.to_string()))?;
    let rows = session
        .text_query(&format!("SHOW CREATE TABLE {relation}"), &[])
        .await?;
    Ok(rows
        .first()
        .map(|row| format!("{};\n", text(row, 1)))
        .unwrap_or_default())
}

fn column_detail(row: &ColumnRow) -> ColumnDetail {
    let generated = row.generated();
    ColumnDetail {
        name: row.name.clone(),
        data_type: row.column_type.clone(),
        nullable: row.nullable,
        default: if generated {
            row.generation.clone()
        } else {
            row.default.clone()
        },
        generated: if generated {
            Generated::Stored
        } else {
            Generated::No
        },
        comment: row.comment.clone(),
    }
}

fn key_constraints(indexes: &[IndexRow]) -> Vec<ConstraintDetail> {
    indexes
        .iter()
        .filter(|index| index.unique)
        .map(|index| {
            let primary = index.name == "PRIMARY";
            let keyword = if primary { "PRIMARY KEY" } else { "UNIQUE" };
            ConstraintDetail {
                name: index.name.clone(),
                kind: if primary {
                    ConstraintKind::Primary
                } else {
                    ConstraintKind::Unique
                },
                definition: format!("{keyword} ({})", index.columns.join(", ")),
                columns: index.columns.clone(),
                references: None,
            }
        })
        .collect()
}

fn index_detail(index: &IndexRow, table: &str) -> IndexDetail {
    let unique = if index.unique { "UNIQUE " } else { "" };
    IndexDetail {
        name: index.name.clone(),
        method: index.method.clone(),
        unique: index.unique,
        primary: index.name == "PRIMARY",
        constraint: index.unique,
        columns: index.columns.join(", "),
        include: Vec::new(),
        predicate: None,
        comment: None,
        definition: format!(
            "CREATE {unique}INDEX {} ON {table} ({}) USING {}",
            index.name,
            index.columns.join(", "),
            index.method.to_ascii_uppercase()
        ),
    }
}

async fn foreign_keys(
    session: &mut MySqlSession,
    schema: &str,
    table: &str,
) -> Result<Vec<ConstraintDetail>, DriverError> {
    let rows = session
        .text_query(FOREIGN_SQL, &bind(&[schema, table]))
        .await?;
    let mut keys: Vec<ConstraintDetail> = Vec::new();
    for row in &rows {
        let name = text(row, 0);
        if keys.last().is_none_or(|key| key.name != name) {
            keys.push(ConstraintDetail {
                name,
                kind: ConstraintKind::Foreign,
                definition: String::new(),
                columns: Vec::new(),
                references: Some(ForeignTarget {
                    schema: text(row, 2),
                    table: text(row, 3),
                    columns: Vec::new(),
                }),
            });
        }
        if let Some(key) = keys.last_mut() {
            key.columns.push(text(row, 1));
            if let Some(target) = key.references.as_mut() {
                target.columns.push(text(row, 4));
            }
        }
    }
    for key in &mut keys {
        let target = key.references.clone().unwrap_or_default();
        key.definition = format!(
            "FOREIGN KEY ({}) REFERENCES {}.{} ({})",
            key.columns.join(", "),
            target.schema,
            target.table,
            target.columns.join(", ")
        );
    }
    Ok(keys)
}

async fn triggers(
    session: &mut MySqlSession,
    schema: &str,
    table: &str,
) -> Result<Vec<TriggerDetail>, DriverError> {
    let rows = session
        .text_query(TRIGGERS_SQL, &bind(&[schema, table]))
        .await?;
    Ok(rows
        .iter()
        .map(|row| {
            let (name, timing, event, body) =
                (text(row, 0), text(row, 1), text(row, 2), text(row, 3));
            TriggerDetail {
                definition: format!(
                    "CREATE TRIGGER {name} {timing} {event} ON {table} FOR EACH ROW {body}"
                ),
                name,
                timing: if timing == "BEFORE" {
                    TriggerTiming::Before
                } else {
                    TriggerTiming::After
                },
                events: match event.as_str() {
                    "INSERT" => vec![TriggerEvent::Insert],
                    "DELETE" => vec![TriggerEvent::Delete],
                    _ => vec![TriggerEvent::Update],
                },
                row_level: true,
                function: String::new(),
                enabled: true,
            }
        })
        .collect())
}
