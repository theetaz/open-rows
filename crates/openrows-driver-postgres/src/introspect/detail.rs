// SPDX-License-Identifier: Apache-2.0
//! Table detail for the Structure, Index, Triggers and DDL views: columns
//! with generation kinds, constraints, full index definitions, triggers,
//! sizes and comment. Five catalog queries on the metadata session.
use super::table::relation_sql;
use super::{flag, malformed, optional, required};
use crate::session::PgSession;
use crate::simple::TextRow;
use openrows_driver_api::DriverError;
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, ForeignTarget, Generated, IndexDetail,
    RelationKind, TableDetail, TableInfo, TriggerDetail, TriggerEvent, TriggerTiming,
};

const COLUMNS_SQL: &str = include_str!("sql/detail-columns.sql");
const CONSTRAINTS_SQL: &str = include_str!("sql/constraints.sql");
const INDEXES_SQL: &str = include_str!("sql/index-detail.sql");
const TRIGGERS_SQL: &str = include_str!("sql/triggers.sql");
const INFO_SQL: &str = include_str!("sql/table-info.sql");

impl PgSession {
    /// Columns, constraints, indexes, triggers, sizes and comment of
    /// `schema`.`name`; `info` is `None` when the relation does not exist.
    ///
    /// # Errors
    /// `Unsupported` for names containing NUL; transport, protocol or server
    /// failures; a malformed row is `Protocol`.
    pub async fn table_detail(
        &mut self,
        schema: &str,
        name: &str,
    ) -> Result<TableDetail, DriverError> {
        let mut read = async |template: &str| {
            self.text_query(&relation_sql(template, schema, name)?)
                .await
        };
        let info = read(INFO_SQL).await?;
        let Some(info) = info.first().map(table_info).transpose()? else {
            return Ok(TableDetail::default());
        };
        Ok(TableDetail {
            columns: rows(&read(COLUMNS_SQL).await?, column)?,
            constraints: rows(&read(CONSTRAINTS_SQL).await?, constraint)?,
            indexes: rows(&read(INDEXES_SQL).await?, index)?,
            triggers: rows(&read(TRIGGERS_SQL).await?, trigger)?,
            info: Some(info),
        })
    }
}

fn rows<T>(
    rows: &[TextRow],
    map: fn(&TextRow) -> Result<T, DriverError>,
) -> Result<Vec<T>, DriverError> {
    rows.iter().map(map).collect()
}

fn column(row: &TextRow) -> Result<ColumnDetail, DriverError> {
    let generated = match optional(row, 4).as_deref() {
        Some("a") => Generated::IdentityAlways,
        Some("d") => Generated::IdentityByDefault,
        Some("s") => Generated::Stored,
        Some("") | None => Generated::No,
        Some(other) => return Err(malformed(&format!("unknown generated kind {other}"))),
    };
    Ok(ColumnDetail {
        name: required(row, 0)?,
        data_type: required(row, 1)?,
        nullable: flag(row, 2)?,
        default: optional(row, 3),
        generated,
        comment: optional(row, 5),
    })
}

fn constraint(row: &TextRow) -> Result<ConstraintDetail, DriverError> {
    let kind = match required(row, 1)?.as_str() {
        "p" => ConstraintKind::Primary,
        "u" => ConstraintKind::Unique,
        "c" => ConstraintKind::Check,
        "f" => ConstraintKind::Foreign,
        "x" => ConstraintKind::Exclusion,
        other => return Err(malformed(&format!("unknown constraint type {other}"))),
    };
    let references = match (optional(row, 4), optional(row, 5)) {
        (Some(schema), Some(table)) => Some(ForeignTarget {
            schema,
            table,
            columns: split_identifiers(&optional(row, 6).unwrap_or_default())?,
        }),
        _ => None,
    };
    Ok(ConstraintDetail {
        name: required(row, 0)?,
        kind,
        definition: required(row, 2)?,
        columns: split_identifiers(&optional(row, 3).unwrap_or_default())?,
        references,
    })
}

fn index(row: &TextRow) -> Result<IndexDetail, DriverError> {
    Ok(IndexDetail {
        name: required(row, 0)?,
        method: required(row, 1)?,
        unique: flag(row, 2)?,
        primary: flag(row, 3)?,
        constraint: flag(row, 4)?,
        columns: optional(row, 5).unwrap_or_default(),
        include: split_identifiers(&optional(row, 6).unwrap_or_default())?,
        predicate: optional(row, 7),
        comment: optional(row, 8),
        definition: required(row, 9)?,
    })
}

/// `pg_trigger.tgtype` bits (`catalog/pg_trigger.h`).
const ROW: u32 = 1;
const BEFORE: u32 = 2;
const INSERT: u32 = 4;
const DELETE: u32 = 8;
const UPDATE: u32 = 16;
const TRUNCATE: u32 = 32;
const INSTEAD: u32 = 64;

fn trigger(row: &TextRow) -> Result<TriggerDetail, DriverError> {
    let bits = number(&required(row, 1)?, "trigger type")?;
    let bits = u32::try_from(bits).map_err(|_| malformed("trigger type out of range"))?;
    let timing = if bits & INSTEAD != 0 {
        TriggerTiming::InsteadOf
    } else if bits & BEFORE != 0 {
        TriggerTiming::Before
    } else {
        TriggerTiming::After
    };
    let events = [
        (INSERT, TriggerEvent::Insert),
        (UPDATE, TriggerEvent::Update),
        (DELETE, TriggerEvent::Delete),
        (TRUNCATE, TriggerEvent::Truncate),
    ]
    .into_iter()
    .filter(|(bit, _)| bits & bit != 0)
    .map(|(_, event)| event)
    .collect();
    Ok(TriggerDetail {
        name: required(row, 0)?,
        timing,
        events,
        row_level: bits & ROW != 0,
        enabled: required(row, 2)? != "D",
        function: required(row, 3)?,
        definition: required(row, 4)?,
    })
}

fn table_info(row: &TextRow) -> Result<TableInfo, DriverError> {
    let kind = match required(row, 0)?.as_str() {
        "r" => RelationKind::Table,
        "p" => RelationKind::PartitionedTable,
        "v" => RelationKind::View,
        "m" => RelationKind::MaterializedView,
        "f" => RelationKind::ForeignTable,
        other => return Err(malformed(&format!("unknown relation kind {other}"))),
    };
    let bytes = |index| {
        number(&required(row, index)?, "size")
            .and_then(|value| u64::try_from(value).map_err(|_| malformed("negative size")))
    };
    Ok(TableInfo {
        kind,
        row_estimate: optional(row, 1)
            .map(|text| text.parse::<u64>())
            .transpose()
            .map_err(|_| malformed("row estimate is not a number"))?,
        total_bytes: bytes(2)?,
        data_bytes: bytes(3)?,
        index_bytes: bytes(4)?,
        comment: optional(row, 5),
        view_definition: optional(row, 6),
        partition_key: optional(row, 7),
        definition: None,
    })
}

fn number(text: &str, what: &str) -> Result<i64, DriverError> {
    text.parse::<i64>()
        .map_err(|_| malformed(&format!("{what} is not a number")))
}

/// Split `quote_ident` names joined by commas, e.g. `id,"a,b","x""y"`.
fn split_identifiers(text: &str) -> Result<Vec<String>, DriverError> {
    let mut names = Vec::new();
    let mut chars = text.chars().peekable();
    while chars.peek().is_some() {
        let mut name = String::new();
        if chars.next_if_eq(&'"').is_some() {
            loop {
                match chars.next() {
                    Some('"') if chars.next_if_eq(&'"').is_some() => name.push('"'),
                    Some('"') => break,
                    Some(other) => name.push(other),
                    None => return Err(malformed("unterminated quoted identifier")),
                }
            }
        } else {
            while let Some(next) = chars.next_if(|next| *next != ',') {
                name.push(next);
            }
        }
        names.push(name);
        if chars.next().is_some_and(|next| next != ',') {
            return Err(malformed("identifier list is malformed"));
        }
    }
    Ok(names)
}

#[cfg(test)]
#[path = "detail_tests.rs"]
mod tests;
