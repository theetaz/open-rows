// SPDX-License-Identifier: Apache-2.0
//! Table detail: columns, key, unique, check and foreign-key constraints,
//! indexes, triggers, sizes, and the DDL (a view's own definition, or a
//! `CREATE TABLE` rebuilt from the catalog with named default constraints,
//! which the structure editor reads back).
use openrows_driver_api::DriverError;
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, ForeignTarget, Generated, IndexDetail,
    RelationKind, TableDetail, TableInfo, TriggerDetail, TriggerEvent, TriggerTiming,
};

use super::rows::{ColumnRow, IndexRow, bind, flag, optional, text};
use crate::ddl::definition::{Relation, create_table, index_definition, quote};
use crate::session::SqlServerSession;

const TABLE_SQL: &str = include_str!("sql/table.sql");
const CONSTRAINTS_SQL: &str = include_str!("sql/constraints.sql");
const TRIGGERS_SQL: &str = include_str!("sql/triggers.sql");

/// Everything the Structure, Index, Triggers and DDL views show.
pub(crate) async fn table_detail(
    session: &mut SqlServerSession,
    schema: &str,
    table: &str,
) -> Result<TableDetail, DriverError> {
    let found = session
        .text_query(TABLE_SQL, &bind(&[schema, table]))
        .await?;
    let Some(row) = found.first() else {
        return Ok(TableDetail::default());
    };
    let view = text(row, 0).trim() == "V";
    let columns = session.column_rows(schema, table).await?;
    let indexes = session.index_rows(schema, table).await?;
    let mut constraints = key_constraints(&indexes);
    constraints.extend(other_constraints(session, schema, table).await?);
    let relation = Relation { schema, table };
    let definition = if view {
        optional(row, 5)
    } else {
        Some(create_table(&relation, &columns, &constraints, &indexes))
    };
    let number = |index: usize| optional(row, index).and_then(|value| value.parse::<u64>().ok());
    let (total, data) = (number(2).unwrap_or(0), number(3).unwrap_or(0));
    let info = TableInfo {
        kind: if view {
            RelationKind::View
        } else {
            RelationKind::Table
        },
        row_estimate: number(1),
        total_bytes: total,
        data_bytes: data,
        index_bytes: total.saturating_sub(data),
        comment: optional(row, 4),
        view_definition: if view { optional(row, 5) } else { None },
        partition_key: None,
        definition,
    };
    Ok(TableDetail {
        columns: columns.iter().map(column_detail).collect(),
        constraints,
        indexes: indexes
            .iter()
            .map(|index| index_detail(&relation, index))
            .collect(),
        triggers: triggers(session, schema, table).await?,
        partitions: Vec::new(),
        info: Some(info),
    })
}

fn column_detail(row: &ColumnRow) -> ColumnDetail {
    let generated = if row.identity.is_some() {
        Generated::IdentityAlways
    } else if row.computed.is_some() {
        Generated::Stored
    } else {
        Generated::No
    };
    ColumnDetail {
        name: row.name.clone(),
        data_type: row.data_type.clone(),
        nullable: row.nullable,
        default: row.computed.clone().or_else(|| row.default.clone()),
        generated,
        comment: row.comment.clone(),
    }
}

/// Primary keys and unique constraints are indexes in SQL Server.
fn key_constraints(indexes: &[IndexRow]) -> Vec<ConstraintDetail> {
    indexes
        .iter()
        .filter(|index| index.primary || index.unique_constraint)
        .map(|index| {
            let keyword = if index.primary {
                "PRIMARY KEY"
            } else {
                "UNIQUE"
            };
            ConstraintDetail {
                name: index.name.clone(),
                kind: if index.primary {
                    ConstraintKind::Primary
                } else {
                    ConstraintKind::Unique
                },
                definition: format!("{keyword} ({})", index.key_list()),
                columns: index.names(),
                references: None,
            }
        })
        .collect()
}

fn index_detail(relation: &Relation<'_>, index: &IndexRow) -> IndexDetail {
    IndexDetail {
        name: index.name.clone(),
        method: index.method.clone(),
        unique: index.unique,
        primary: index.primary,
        constraint: index.primary || index.unique_constraint,
        columns: index.key_list(),
        include: index.include.clone(),
        predicate: index.filter.clone(),
        comment: None,
        definition: index_definition(relation, index),
    }
}

/// `CHECK` constraints, then foreign keys with their column pairs.
async fn other_constraints(
    session: &mut SqlServerSession,
    schema: &str,
    table: &str,
) -> Result<Vec<ConstraintDetail>, DriverError> {
    let rows = session
        .text_query(CONSTRAINTS_SQL, &bind(&[schema, table]))
        .await?;
    let mut found: Vec<ConstraintDetail> = Vec::new();
    for row in &rows {
        let (name, foreign) = (text(row, 1), text(row, 0) == "F");
        if found.last().is_none_or(|last| last.name != name) {
            found.push(ConstraintDetail {
                name,
                kind: if foreign {
                    ConstraintKind::Foreign
                } else {
                    ConstraintKind::Check
                },
                definition: format!("CHECK {}", text(row, 2)),
                columns: Vec::new(),
                references: foreign.then(|| ForeignTarget {
                    schema: text(row, 4),
                    table: text(row, 5),
                    columns: Vec::new(),
                }),
            });
        }
        if let Some(constraint) = found.last_mut() {
            constraint.columns.extend(optional(row, 3));
            if let Some(target) = constraint.references.as_mut() {
                target.columns.push(text(row, 6));
            }
        }
    }
    for constraint in &mut found {
        if let Some(target) = &constraint.references {
            constraint.definition = foreign_definition(&constraint.columns, target);
        }
    }
    Ok(found)
}

/// `FOREIGN KEY ([a]) REFERENCES [s].[t] ([b])`.
pub(crate) fn foreign_definition(columns: &[String], target: &ForeignTarget) -> String {
    let list = |names: &[String]| {
        names
            .iter()
            .map(|name| quote(name))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "FOREIGN KEY ({}) REFERENCES {}.{} ({})",
        list(columns),
        quote(&target.schema),
        quote(&target.table),
        list(&target.columns)
    )
}

async fn triggers(
    session: &mut SqlServerSession,
    schema: &str,
    table: &str,
) -> Result<Vec<TriggerDetail>, DriverError> {
    let rows = session
        .text_query(TRIGGERS_SQL, &bind(&[schema, table]))
        .await?;
    Ok(rows
        .iter()
        .map(|row| TriggerDetail {
            name: text(row, 0),
            timing: if flag(row, 1) {
                TriggerTiming::InsteadOf
            } else {
                TriggerTiming::After
            },
            events: text(row, 3)
                .split(',')
                .filter_map(|event| match event.trim() {
                    "INSERT" => Some(TriggerEvent::Insert),
                    "UPDATE" => Some(TriggerEvent::Update),
                    "DELETE" => Some(TriggerEvent::Delete),
                    _ => None,
                })
                .collect(),
            row_level: false,
            function: String::new(),
            enabled: !flag(row, 2),
            definition: text(row, 4),
        })
        .collect())
}
