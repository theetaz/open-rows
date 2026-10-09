// SPDX-License-Identifier: Apache-2.0
//! Warnings that need the connection or the data, and the live DDL preview
//! (`table_alter_preview`) the Structure view shows while changes are staged.
use super::dto::TableTarget;
use super::schema_dto::SchemaChangeDto;
use crate::connections::catalog::read_detail;
use crate::connections::drivers::profile_driver;
use crate::connections::profiles::profile_id;
use crate::connections::{Connections, IpcError};
use arrow_array::{Array, Int32Array, Int64Array, RecordBatch, UInt64Array};
use arrow_schema::SchemaRef;
use openrows_driver_api::schema::SchemaChange;
use openrows_driver_api::structure::AlterTable;
use openrows_driver_api::{Connection, ResultSink, SinkFlow};
use openrows_sql::Dialect;
use openrows_store::profile::EnvKind;
use serde::Serialize;
use specta::Type;
use tauri::State;

/// Most NULLs counted per column; a larger count shows as "at least".
const NULL_LIMIT: u64 = 1000;

/// The review warning on production connections.
pub(crate) const PRODUCTION: &str =
    "This is a production connection: check every statement before committing.";

/// Generated DDL for a change set, without committing anything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlterPreviewDto {
    pub(crate) statements: Vec<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) destructive: bool,
}

/// The DDL a staged change set would run, read against the catalog on the
/// metadata session; nothing runs and no review token is issued.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_alter_preview(
    state: State<'_, Connections>,
    target: TableTarget,
    changes: Vec<SchemaChangeDto>,
) -> Result<AlterPreviewDto, IpcError> {
    let profile = state.profiles.get(profile_id(&target.profile_id)?)?;
    let driver = profile_driver(&state, &target.profile_id)?;
    let detail = read_detail(
        &state,
        (&target.profile_id, &target.database),
        (target.schema.clone(), target.table.clone()),
    )
    .await?;
    let changes: Vec<SchemaChange> = changes.iter().map(Into::into).collect();
    let plan = driver
        .alter_sql(
            &AlterTable {
                schema: &target.schema,
                table: &target.table,
                detail: &detail,
            },
            &changes,
        )
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    let mut warnings = plan.warnings;
    if profile.env_tag.kind == EnvKind::Prod {
        warnings.push(PRODUCTION.to_owned());
    }
    Ok(AlterPreviewDto {
        statements: plan.statements,
        warnings,
        destructive: plan.destructive,
    })
}

/// How many NULLs each column that gains `NOT NULL` holds, as warnings.
pub(crate) async fn null_warnings(
    session: &mut dyn Connection,
    dialect: Dialect,
    target: &TableTarget,
    changes: &[SchemaChange],
) -> Vec<String> {
    let mut warnings = Vec::new();
    for change in changes {
        let SchemaChange::ColumnNullable {
            column,
            nullable: false,
        } = change
        else {
            continue;
        };
        let Ok(sql) = null_count_sql(dialect, target, column) else {
            continue;
        };
        let mut count = Count::default();
        let warning = match session.execute(&sql, &mut count).await {
            Ok(_) => nulls_warning(column, count.value),
            Err(error) => Some(format!(
                "The NULLs in {column:?} could not be counted: {error}"
            )),
        };
        warnings.extend(warning);
    }
    warnings
}

/// `SELECT COUNT(*)` of at most [`NULL_LIMIT`] + 1 NULL rows of `column`.
fn null_count_sql(dialect: Dialect, target: &TableTarget, column: &str) -> Result<String, String> {
    let relation = dialect
        .qualified(&target.schema, &target.table)
        .map_err(|error| error.to_string())?;
    let column = dialect
        .quote_ident(column)
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "SELECT COUNT(*) FROM (SELECT 1 AS one FROM {relation} WHERE {column} IS NULL LIMIT {}) AS nulls",
        NULL_LIMIT + 1
    ))
}

fn nulls_warning(column: &str, count: Option<u64>) -> Option<String> {
    let count = count.filter(|count| *count > 0)?;
    let amount = if count > NULL_LIMIT {
        format!("more than {NULL_LIMIT}")
    } else {
        count.to_string()
    };
    Some(format!(
        "{column:?} holds {amount} NULL values: NOT NULL fails until they are filled."
    ))
}

/// The first value of a one-column integer result.
#[derive(Default)]
struct Count {
    value: Option<u64>,
}

impl ResultSink for Count {
    fn begin_result(&mut self, _schema: SchemaRef) {}

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        if self.value.is_none()
            && let Some(column) = batch.columns().first()
            && !column.is_empty()
            && !column.is_null(0)
        {
            let any = column.as_any();
            self.value = any
                .downcast_ref::<Int64Array>()
                .and_then(|array| u64::try_from(array.value(0)).ok())
                .or_else(|| {
                    any.downcast_ref::<UInt64Array>()
                        .map(|array| array.value(0))
                })
                .or_else(|| {
                    any.downcast_ref::<Int32Array>()
                        .and_then(|array| u64::try_from(array.value(0)).ok())
                });
        }
        SinkFlow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_counts_are_bounded_and_quoted() -> Result<(), String> {
        let target = TableTarget {
            profile_id: "p".into(),
            database: "d".into(),
            schema: "shop".into(),
            table: "orders".into(),
        };
        assert_eq!(
            null_count_sql(Dialect::MySql, &target, "note")?,
            "SELECT COUNT(*) FROM (SELECT 1 AS one FROM `shop`.`orders` WHERE `note` IS NULL LIMIT 1001) AS nulls"
        );
        assert_eq!(nulls_warning("note", Some(0)), None);
        assert_eq!(
            nulls_warning("note", Some(3)).as_deref(),
            Some("\"note\" holds 3 NULL values: NOT NULL fails until they are filled.")
        );
        assert!(nulls_warning("note", Some(1001)).is_some_and(|w| w.contains("more than 1000")));
        Ok(())
    }
}
