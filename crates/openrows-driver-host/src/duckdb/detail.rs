// SPDX-License-Identifier: Apache-2.0
//! Relation detail and read-only object inspection on the shared native session.
use super::catalog_sql as sql;
use super::session::DuckdbSession;
use super::text::{self, Row};
use openrows_driver_api::DriverError;
use openrows_driver_api::catalog::{IndexSummary, ObjectKind, TableStructure};
use openrows_driver_api::inspection::{ObjectDetail, ObjectIdentity, attributes, view_detail};
use openrows_driver_api::schema::{
    ColumnDetail, ConstraintDetail, ConstraintKind, ForeignTarget, IndexDetail, RelationKind,
    TableDetail, TableInfo,
};

impl DuckdbSession {
    pub(super) fn structure(
        &self,
        schema: &str,
        table: &str,
    ) -> Result<TableStructure, DriverError> {
        let detail = self.detail(schema, table)?;
        Ok(TableStructure {
            columns: self.columns(schema, table)?,
            indexes: detail
                .indexes
                .into_iter()
                .map(|index| IndexSummary {
                    name: index.name,
                    columns: index.columns,
                    unique: index.unique,
                    primary: index.primary,
                    method: index.method,
                    predicate: index.predicate,
                })
                .collect(),
            row_estimate: detail.info.and_then(|info| info.row_estimate),
        })
    }

    pub(super) fn detail(&self, schema: &str, table: &str) -> Result<TableDetail, DriverError> {
        let target = [Some(schema), Some(table)];
        let Some(info) = self.query(sql::RELATION, &target, 4)?.first().map(info) else {
            return Ok(TableDetail::default());
        };
        let columns = self
            .columns(schema, table)?
            .into_iter()
            .map(|column| ColumnDetail {
                name: column.name,
                data_type: column.data_type,
                nullable: column.nullable,
                default: column.default,
                generated: openrows_driver_api::schema::Generated::No,
                comment: column.comment,
            })
            .collect();
        let constraints = self
            .query(sql::CONSTRAINTS, &target, 6)?
            .iter()
            .map(|row| constraint(row, schema, table))
            .collect::<Result<Vec<_>, _>>()?;
        let mut indexes = self
            .query(sql::INDEXES, &target, 6)?
            .iter()
            .map(index)
            .collect::<Result<Vec<_>, _>>()?;
        indexes.extend(constraints.iter().filter_map(constraint_index));
        indexes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(TableDetail {
            columns,
            constraints,
            indexes,
            triggers: Vec::new(),
            partitions: Vec::new(),
            info: Some(info),
        })
    }

    pub(super) fn inspect(&self, object: &ObjectIdentity) -> Result<ObjectDetail, DriverError> {
        let target = [Some(object.schema.as_str()), Some(object.name.as_str())];
        match object.kind {
            ObjectKind::View => view_detail(self.detail(&object.schema, &object.name)?),
            ObjectKind::Sequence => {
                let row = self.query(sql::SEQUENCE, &target, 8)?.into_iter().next();
                let labels = [
                    "Start",
                    "Minimum",
                    "Maximum",
                    "Increment",
                    "Cycle",
                    "Last value",
                ];
                Ok(described(&labels, &row.ok_or_else(missing)?, 7))
            }
            ObjectKind::Function => {
                let wanted = object.arguments.as_deref();
                let row = self
                    .query(sql::MACRO, &target, 5)?
                    .into_iter()
                    .find(|row| wanted.is_none_or(|arguments| same(row, arguments)));
                let labels = ["Type", "Parameters", "Return type", "Comment"];
                Ok(described(&labels, &row.ok_or_else(missing)?, 4))
            }
            _ => Err(DriverError::Unsupported("this DuckDB object viewer".into())),
        }
    }
}

/// Labelled attributes plus the stored definition at `definition`.
fn described(labels: &[&str], row: &Row, definition: usize) -> ObjectDetail {
    ObjectDetail {
        attributes: attributes(labels, row),
        sections: Vec::new(),
        definition: text::optional(row, definition),
    }
}

/// The macro overload whose parameters match the requested identity arguments.
fn same(row: &Row, arguments: &str) -> bool {
    text::optional(row, 1).as_deref() == Some(arguments)
}

fn info(row: &Row) -> TableInfo {
    let view = row.first().and_then(Option::as_deref) == Some("view");
    let definition = text::optional(row, 3);
    TableInfo {
        kind: if view {
            RelationKind::View
        } else {
            RelationKind::Table
        },
        row_estimate: text::optional(row, 1).and_then(|value| value.parse().ok()),
        comment: text::optional(row, 2),
        view_definition: definition.clone().filter(|_| view),
        definition,
        ..TableInfo::default()
    }
}

fn constraint(row: &Row, schema: &str, table: &str) -> Result<ConstraintDetail, DriverError> {
    let kind = match text::value(row, 1)?.as_str() {
        "PRIMARY KEY" => ConstraintKind::Primary,
        "UNIQUE" => ConstraintKind::Unique,
        "FOREIGN KEY" => ConstraintKind::Foreign,
        _ => ConstraintKind::Check,
    };
    let columns = text::names(row, 3);
    let name = text::optional(row, 0).unwrap_or_else(|| format!("{table}_{}", columns.join("_")));
    let references = (kind == ConstraintKind::Foreign).then(|| ForeignTarget {
        schema: schema.to_owned(),
        table: text::optional(row, 4).unwrap_or_default(),
        columns: text::names(row, 5),
    });
    Ok(ConstraintDetail {
        name,
        kind,
        definition: text::optional(row, 2).unwrap_or_default(),
        columns,
        references,
    })
}

fn index(row: &Row) -> Result<IndexDetail, DriverError> {
    Ok(IndexDetail {
        name: text::value(row, 0)?,
        method: "art".into(),
        unique: text::flag(row, 1),
        primary: text::flag(row, 2),
        constraint: false,
        columns: text::optional(row, 3).unwrap_or_default(),
        include: Vec::new(),
        predicate: None,
        comment: text::optional(row, 4),
        definition: text::optional(row, 5).unwrap_or_default(),
    })
}

/// `DuckDB` backs primary-key and unique constraints with implicit ART indexes.
fn constraint_index(constraint: &ConstraintDetail) -> Option<IndexDetail> {
    let primary = constraint.kind == ConstraintKind::Primary;
    (primary || constraint.kind == ConstraintKind::Unique).then(|| IndexDetail {
        name: constraint.name.clone(),
        method: "art".into(),
        unique: true,
        primary,
        constraint: true,
        columns: constraint.columns.join(", "),
        include: Vec::new(),
        predicate: None,
        comment: None,
        definition: constraint.definition.clone(),
    })
}

fn missing() -> DriverError {
    DriverError::Server {
        code: "42704".into(),
        message: "Object no longer exists or is not visible".into(),
    }
}
