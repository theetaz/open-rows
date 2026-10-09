// SPDX-License-Identifier: Apache-2.0
//! Table detail from the pragma functions and `sqlite_master`: columns,
//! key, unique and foreign-key constraints, indexes, triggers and the
//! stored `CREATE` statements for the DDL view.
use openrows_driver_api::DriverError;
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, ForeignTarget, Generated, IndexDetail,
    RelationKind, TableDetail, TableInfo, TriggerDetail, TriggerEvent, TriggerTiming,
};
use openrows_sql::Dialect;
use rusqlite::{Connection, OptionalExtension, params};

use super::{ColumnRow, column_rows, invalid, sql};

/// `sqlite_master` of `schema`, quoted.
fn master(schema: &str) -> Result<String, DriverError> {
    Dialect::Sqlite
        .qualified(schema, "sqlite_master")
        .map_err(|error| invalid(&error))
}

/// Columns, constraints, indexes, triggers and stored SQL of `schema.table`.
pub(crate) fn table_detail(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<TableDetail, DriverError> {
    let query = format!(
        "SELECT type, sql FROM {} WHERE name = ?1 AND type IN ('table', 'view')",
        master(schema)?
    );
    let found: Option<(String, Option<String>)> = sql(connection
        .query_row(&query, params![table], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional())?;
    let Some((kind, create)) = found else {
        return Ok(TableDetail::default());
    };
    let rows = column_rows(connection, schema, table)?;
    let indexes = indexes(connection, schema, table)?;
    let triggers = triggers(connection, schema, table)?;
    let mut constraints = primary_key(&rows).into_iter().collect::<Vec<_>>();
    constraints.extend(unique_constraints(&indexes));
    constraints.extend(foreign_keys(connection, schema, table)?);
    let definition = definition(create.as_deref(), &indexes, &triggers);
    let view = kind == "view";
    Ok(TableDetail {
        columns: rows.iter().map(column_detail).collect(),
        constraints,
        indexes,
        triggers,
        partitions: Vec::new(),
        info: Some(TableInfo {
            kind: if view {
                RelationKind::View
            } else {
                RelationKind::Table
            },
            view_definition: if view { create } else { None },
            definition: Some(definition),
            ..TableInfo::default()
        }),
    })
}

fn column_detail(row: &ColumnRow) -> ColumnDetail {
    ColumnDetail {
        name: row.name.clone(),
        data_type: row.data_type.clone(),
        nullable: !row.not_null && row.primary_key == 0,
        default: row.default.clone(),
        // 2: generated virtual, 3: generated stored (`PRAGMA table_xinfo`).
        generated: if row.hidden >= 2 {
            Generated::Stored
        } else {
            Generated::No
        },
        comment: None,
    }
}

fn primary_key(rows: &[ColumnRow]) -> Option<ConstraintDetail> {
    let mut keyed: Vec<&ColumnRow> = rows.iter().filter(|row| row.primary_key > 0).collect();
    keyed.sort_by_key(|row| row.primary_key);
    let columns: Vec<String> = keyed.iter().map(|row| row.name.clone()).collect();
    (!columns.is_empty()).then(|| ConstraintDetail {
        name: "PRIMARY KEY".to_owned(),
        kind: ConstraintKind::Primary,
        definition: format!("PRIMARY KEY ({})", columns.join(", ")),
        columns,
        references: None,
    })
}

fn unique_constraints(indexes: &[IndexDetail]) -> Vec<ConstraintDetail> {
    indexes
        .iter()
        .filter(|index| index.unique && index.constraint && !index.primary)
        .map(|index| ConstraintDetail {
            name: index.name.clone(),
            kind: ConstraintKind::Unique,
            definition: format!("UNIQUE ({})", index.columns),
            columns: index.columns.split(", ").map(str::to_owned).collect(),
            references: None,
        })
        .collect()
}

type ForeignRow = (i64, String, String, Option<String>);

fn foreign_keys(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<Vec<ConstraintDetail>, DriverError> {
    let mut statement = sql(connection.prepare(
        "SELECT id, \"table\", \"from\", \"to\" FROM pragma_foreign_key_list(?1, ?2) ORDER BY id, seq",
    ))?;
    let rows = sql(statement.query_map(params![table, schema], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    }))?;
    let rows: Vec<ForeignRow> = rows.map(sql).collect::<Result<_, _>>()?;
    let mut keys: Vec<ConstraintDetail> = Vec::new();
    for (id, target, from, to) in rows {
        let name = format!("fk_{table}_{id}");
        if keys.last().is_none_or(|key| key.name != name) {
            keys.push(foreign(name, schema, target));
        }
        if let Some(key) = keys.last_mut() {
            key.columns.push(from);
            if let Some(references) = key.references.as_mut() {
                references.columns.push(to.unwrap_or_default());
            }
        }
    }
    for key in &mut keys {
        let references = key.references.clone().unwrap_or_default();
        key.definition = format!(
            "FOREIGN KEY ({}) REFERENCES {} ({})",
            key.columns.join(", "),
            references.table,
            references.columns.join(", ")
        );
    }
    Ok(keys)
}

fn foreign(name: String, schema: &str, table: String) -> ConstraintDetail {
    ConstraintDetail {
        name,
        kind: ConstraintKind::Foreign,
        definition: String::new(),
        columns: Vec::new(),
        references: Some(ForeignTarget {
            schema: schema.to_owned(),
            table,
            columns: Vec::new(),
        }),
    }
}

type IndexRow = (String, bool, String, bool);

/// Indexes of `schema.table` ordered by name.
pub(crate) fn indexes(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<Vec<IndexDetail>, DriverError> {
    let mut statement = sql(connection.prepare(
        "SELECT name, \"unique\", origin, partial FROM pragma_index_list(?1, ?2) ORDER BY name",
    ))?;
    let rows = sql(statement.query_map(params![table, schema], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    }))?;
    let rows: Vec<IndexRow> = rows.map(sql).collect::<Result<_, _>>()?;
    rows.into_iter()
        .map(|(name, unique, origin, partial)| {
            let definition = stored_sql(connection, schema, "index", &name)?;
            Ok(IndexDetail {
                columns: index_columns(connection, schema, &name)?,
                method: "btree".to_owned(),
                unique,
                primary: origin == "pk",
                constraint: origin != "c",
                include: Vec::new(),
                predicate: partial.then(|| predicate(definition.as_deref())).flatten(),
                comment: None,
                definition: definition.unwrap_or_else(|| format!("-- automatic index {name}")),
                name,
            })
        })
        .collect()
}

fn index_columns(
    connection: &Connection,
    schema: &str,
    index: &str,
) -> Result<String, DriverError> {
    let mut statement = sql(connection.prepare(
        "SELECT coalesce(name, '<expression>') FROM pragma_index_info(?1, ?2) ORDER BY seqno",
    ))?;
    let names = sql(statement.query_map(params![index, schema], |row| row.get::<_, String>(0)))?;
    Ok(names.map(sql).collect::<Result<Vec<_>, _>>()?.join(", "))
}

/// The text after the last top-level `WHERE` of a partial index.
fn predicate(definition: Option<&str>) -> Option<String> {
    let definition = definition?;
    let upper = definition.to_ascii_uppercase();
    upper
        .rfind(" WHERE ")
        .and_then(|at| definition.get(at + " WHERE ".len()..))
        .map(|text| text.trim().to_owned())
}

/// `sqlite_master.sql` of one object; `None` for automatic indexes.
fn stored_sql(
    connection: &Connection,
    schema: &str,
    kind: &str,
    name: &str,
) -> Result<Option<String>, DriverError> {
    let query = format!(
        "SELECT sql FROM {} WHERE type = ?1 AND name = ?2",
        master(schema)?
    );
    let found: Option<Option<String>> = sql(connection
        .query_row(&query, params![kind, name], |row| row.get(0))
        .optional())?;
    Ok(found.flatten())
}

fn triggers(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<Vec<TriggerDetail>, DriverError> {
    let query = format!(
        "SELECT name, sql FROM {} WHERE type = 'trigger' AND tbl_name = ?1 ORDER BY name",
        master(schema)?
    );
    let mut statement = sql(connection.prepare(&query))?;
    let rows = sql(statement.query_map(params![table], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    }))?;
    rows.map(|row| sql(row).map(|(name, text)| trigger(name, text.unwrap_or_default())))
        .collect()
}

/// Timing and events read from the trigger's header (`… BEFORE UPDATE OF a ON t …`).
fn trigger(name: String, definition: String) -> TriggerDetail {
    let upper = definition.to_ascii_uppercase();
    let header = upper.split(" ON ").next().unwrap_or_default();
    let timing = if header.contains(" INSTEAD OF ") {
        TriggerTiming::InsteadOf
    } else if header.contains(" BEFORE ") {
        TriggerTiming::Before
    } else {
        TriggerTiming::After
    };
    let events = [
        (" INSERT", TriggerEvent::Insert),
        (" UPDATE", TriggerEvent::Update),
        (" DELETE", TriggerEvent::Delete),
    ]
    .into_iter()
    .filter(|(word, _)| header.contains(word))
    .map(|(_, event)| event)
    .collect();
    TriggerDetail {
        name,
        timing,
        events,
        row_level: true,
        function: String::new(),
        enabled: true,
        definition,
    }
}

/// The DDL view: the stored `CREATE` statements, table first.
fn definition(create: Option<&str>, indexes: &[IndexDetail], triggers: &[TriggerDetail]) -> String {
    let mut statements: Vec<String> = create.map(str::to_owned).into_iter().collect();
    statements.extend(
        indexes
            .iter()
            .filter(|index| !index.definition.starts_with("--"))
            .map(|index| index.definition.clone()),
    );
    statements.extend(triggers.iter().map(|trigger| trigger.definition.clone()));
    statements
        .iter()
        .fold(String::new(), |mut text, statement| {
            text.push_str(statement);
            text.push_str(";\n");
            text
        })
}
