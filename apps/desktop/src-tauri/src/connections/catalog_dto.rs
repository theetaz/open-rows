// SPDX-License-Identifier: Apache-2.0
//! Webview mirrors of the driver catalog model (docs/10 §2).
use openrows_driver_api::catalog::{DatabaseSummary, ObjectKind, ObjectSummary, SchemaSummary};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Object kinds the schema tree lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ObjectKindDto {
    Table,
    View,
    MaterializedView,
    ForeignTable,
    Function,
    Procedure,
    Sequence,
}

impl From<ObjectKind> for ObjectKindDto {
    fn from(kind: ObjectKind) -> Self {
        match kind {
            ObjectKind::Table => Self::Table,
            ObjectKind::View => Self::View,
            ObjectKind::MaterializedView => Self::MaterializedView,
            ObjectKind::ForeignTable => Self::ForeignTable,
            ObjectKind::Function => Self::Function,
            ObjectKind::Procedure => Self::Procedure,
            ObjectKind::Sequence => Self::Sequence,
        }
    }
}

impl From<ObjectKindDto> for ObjectKind {
    fn from(kind: ObjectKindDto) -> Self {
        match kind {
            ObjectKindDto::Table => Self::Table,
            ObjectKindDto::View => Self::View,
            ObjectKindDto::MaterializedView => Self::MaterializedView,
            ObjectKindDto::ForeignTable => Self::ForeignTable,
            ObjectKindDto::Function => Self::Function,
            ObjectKindDto::Procedure => Self::Procedure,
            ObjectKindDto::Sequence => Self::Sequence,
        }
    }
}

/// A database on the server.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DatabaseDto {
    name: String,
    /// Maintenance or provider database, hidden unless "Show system databases".
    system: bool,
    comment: Option<String>,
}

impl From<DatabaseSummary> for DatabaseDto {
    fn from(database: DatabaseSummary) -> Self {
        Self {
            name: database.name,
            system: database.system,
            comment: database.comment,
        }
    }
}

/// A schema of the browsed database.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchemaDto {
    name: String,
    /// `pg_catalog` or `information_schema`, hidden unless "Show system schemas".
    system: bool,
    comment: Option<String>,
}

impl From<SchemaSummary> for SchemaDto {
    fn from(schema: SchemaSummary) -> Self {
        Self {
            name: schema.name,
            system: schema.system,
            comment: schema.comment,
        }
    }
}

/// One object; its path is `[database, schema, name]`.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObjectDto {
    schema: String,
    name: String,
    kind: ObjectKindDto,
    /// Routine identity arguments, e.g. `integer, text`.
    arguments: Option<String>,
    /// Planner estimate (`pg_class.reltuples`), never an exact count.
    row_estimate: Option<f64>,
    comment: Option<String>,
}

impl From<ObjectSummary> for ObjectDto {
    #[expect(
        clippy::cast_precision_loss,
        reason = "row estimates are approximate; f64 keeps 53 bits, beyond any real table"
    )]
    fn from(object: ObjectSummary) -> Self {
        Self {
            schema: object.schema,
            name: object.name,
            kind: object.kind.into(),
            arguments: object.arguments,
            row_estimate: object.row_estimate.map(|rows| rows as f64),
            comment: object.comment,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_and_use_camel_case() -> Result<(), serde_json::Error> {
        for kind in ObjectKind::ALL {
            assert_eq!(ObjectKind::from(ObjectKindDto::from(kind)), kind);
        }
        let json = serde_json::to_value(ObjectKindDto::MaterializedView)?;
        assert_eq!(json, "materializedView");
        Ok(())
    }

    #[test]
    fn objects_serialize_with_estimates_and_arguments() -> Result<(), serde_json::Error> {
        let table = ObjectDto::from(ObjectSummary {
            schema: "public".to_owned(),
            name: "orders".to_owned(),
            kind: ObjectKind::Table,
            arguments: None,
            row_estimate: Some(2_400_000),
            comment: Some("Customer orders".to_owned()),
        });
        let json = serde_json::to_value(table)?;
        assert_eq!(json["kind"], "table");
        assert_eq!(json["rowEstimate"], 2_400_000.0);
        assert_eq!(json["comment"], "Customer orders");
        assert!(json["arguments"].is_null());
        Ok(())
    }

    #[test]
    fn databases_and_schemas_keep_the_system_flag() -> Result<(), serde_json::Error> {
        let database = DatabaseDto::from(DatabaseSummary {
            name: "postgres".to_owned(),
            system: true,
            comment: None,
        });
        assert_eq!(serde_json::to_value(database)?["system"], true);
        let schema = SchemaDto::from(SchemaSummary {
            name: "sales".to_owned(),
            system: false,
            comment: Some("Sales data".to_owned()),
        });
        let json = serde_json::to_value(schema)?;
        assert_eq!(
            (json["system"].clone(), json["name"].clone()),
            (false.into(), "sales".into())
        );
        Ok(())
    }
}
