// SPDX-License-Identifier: Apache-2.0
//! Turn an import request into a checked mapping and load plan: target
//! columns must exist in the table (or be defined for a new one), each
//! gets the value category of its type, and a new table gets its DDL.
use std::collections::HashMap;

use openrows_import::{
    ColumnSource, ConflictPolicy, LoadPlan, MappedColumn, Mapper, NewColumn, TableName,
    TypeCategory, create_table_sql,
};
use openrows_sql::Dialect;

use super::dto::{ImportRequest, ImportSourceDto};
use crate::connections::IpcError;

/// Most rows per batch.
const MAX_BATCH: u32 = 100_000;

fn invalid(error: &openrows_import::ImportError) -> IpcError {
    IpcError::invalid(error.to_string())
}

/// The mapping and plan of `request` for a file with `width` columns;
/// `existing` lists the table's columns and types (empty for a new table).
pub(crate) fn plan(
    request: &ImportRequest,
    dialect: Dialect,
    width: usize,
    existing: &[(String, String)],
) -> Result<(LoadPlan, Mapper), IpcError> {
    let types = target_types(request, existing)?;
    let columns: Vec<MappedColumn> = request
        .columns
        .iter()
        .zip(&types)
        .map(|(mapping, data_type)| MappedColumn {
            target: mapping.target.clone(),
            source: match &mapping.source {
                ImportSourceDto::Column { index } => {
                    ColumnSource::Column(usize::try_from(*index).unwrap_or(usize::MAX))
                }
                ImportSourceDto::Constant { value } => ColumnSource::Constant(value.clone()),
            },
            category: TypeCategory::of_type(data_type),
        })
        .collect();
    let mapper = Mapper::new(columns, dialect, width).map_err(|error| invalid(&error))?;
    let table = TableName {
        schema: request.target.schema.clone(),
        table: request.target.table.clone(),
    };
    let create = request
        .create
        .as_ref()
        .map(|create| {
            let columns: Vec<NewColumn> = request
                .columns
                .iter()
                .zip(&create.types)
                .map(|(mapping, data_type)| NewColumn {
                    name: mapping.target.clone(),
                    data_type: data_type.clone(),
                })
                .collect();
            create_table_sql(dialect, &table, &columns, &create.primary_key)
                .map_err(|error| invalid(&error))
        })
        .transpose()?;
    let key = match &request.create {
        Some(create) => create.primary_key.clone(),
        None => checked_key(&request.key, existing)?,
    };
    let plan = LoadPlan {
        dialect,
        table,
        columns: mapper.targets(),
        key,
        // A new table is empty: nothing can conflict.
        conflict: if create.is_some() {
            ConflictPolicy::Error
        } else {
            request.conflict.into()
        },
        truncate: request.truncate && create.is_none(),
        create,
        transaction: request.transaction.into(),
        bad_rows: request.bad_rows.into(),
        mode: request.mode.into(),
    };
    Ok((plan, mapper))
}

/// Rows per batch within 1–100 000.
pub(crate) fn batch_rows(requested: u32) -> usize {
    usize::try_from(requested.clamp(1, MAX_BATCH)).unwrap_or(1)
}

/// The type of each mapped target column.
fn target_types(
    request: &ImportRequest,
    existing: &[(String, String)],
) -> Result<Vec<String>, IpcError> {
    if let Some(create) = &request.create {
        if create.types.len() != request.columns.len() {
            return Err(IpcError::invalid("Every new column needs a type"));
        }
        return Ok(create.types.clone());
    }
    let types: HashMap<&str, &str> = existing
        .iter()
        .map(|(name, data_type)| (name.as_str(), data_type.as_str()))
        .collect();
    request
        .columns
        .iter()
        .map(|mapping| {
            types
                .get(mapping.target.as_str())
                .map(|data_type| (*data_type).to_owned())
                .ok_or_else(|| {
                    IpcError::invalid(format!(
                        "{} has no column {:?}",
                        request.target.table, mapping.target
                    ))
                })
        })
        .collect()
}

fn checked_key(key: &[String], existing: &[(String, String)]) -> Result<Vec<String>, IpcError> {
    match key
        .iter()
        .find(|name| !existing.iter().any(|(column, _)| column == *name))
    {
        Some(missing) => Err(IpcError::invalid(format!(
            "Key column {missing:?} is not a column of the table"
        ))),
        None => Ok(key.to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imports::dto::{
        BadRowsDto, ConflictDto, ImportCreateDto, ImportFormatDto, ImportMappingDto, ImportModeDto,
        ImportOptionsDto, TransactionDto,
    };
    use crate::tables::TableTarget;
    use openrows_import::{RunMode, TransactionMode};

    fn request() -> ImportRequest {
        ImportRequest {
            session_id: "s".to_owned(),
            target: TableTarget {
                profile_id: "p".to_owned(),
                database: "d".to_owned(),
                schema: "public".to_owned(),
                table: "people".to_owned(),
            },
            file_id: "f".to_owned(),
            options: ImportOptionsDto {
                format: ImportFormatDto::Delimited,
                delimiter: ",".to_owned(),
                quote: Some("\"".to_owned()),
                header: true,
                encoding: "utf-8".to_owned(),
                empty_as_null: true,
            },
            columns: vec![
                ImportMappingDto {
                    target: "id".to_owned(),
                    source: ImportSourceDto::Column { index: 0 },
                },
                ImportMappingDto {
                    target: "origin".to_owned(),
                    source: ImportSourceDto::Constant {
                        value: Some("csv".to_owned()),
                    },
                },
            ],
            create: None,
            key: vec!["id".to_owned()],
            conflict: ConflictDto::Update,
            truncate: true,
            transaction: TransactionDto::Batches,
            bad_rows: BadRowsDto::Skip,
            mode: ImportModeDto::Check,
            batch_size: 0,
        }
    }

    fn table() -> Vec<(String, String)> {
        vec![
            ("id".to_owned(), "integer".to_owned()),
            ("origin".to_owned(), "text".to_owned()),
        ]
    }

    #[test]
    fn existing_tables_check_columns_and_keys() -> Result<(), IpcError> {
        let (plan, mapper) = plan(&request(), Dialect::Postgres, 1, &table())?;
        assert_eq!(mapper.targets(), ["id", "origin"]);
        assert_eq!(plan.key, ["id"]);
        assert_eq!(plan.conflict, ConflictPolicy::Update);
        assert_eq!(plan.transaction, TransactionMode::PerBatch);
        assert_eq!(plan.mode, RunMode::Check);
        assert!(plan.truncate && plan.create.is_none());
        let mut unknown = request();
        unknown.columns[1].target = "missing".to_owned();
        assert!(super::plan(&unknown, Dialect::Postgres, 1, &table()).is_err());
        let mut bad_key = request();
        bad_key.key = vec!["nope".to_owned()];
        assert!(super::plan(&bad_key, Dialect::Postgres, 1, &table()).is_err());
        assert!(super::plan(&request(), Dialect::Postgres, 0, &table()).is_err());
        assert_eq!((batch_rows(0), batch_rows(u32::MAX)), (1, 100_000));
        Ok(())
    }

    #[test]
    fn new_tables_get_ddl_and_never_truncate_or_upsert() -> Result<(), IpcError> {
        let mut create = request();
        create.create = Some(ImportCreateDto {
            types: vec!["bigint".to_owned(), "text".to_owned()],
            primary_key: vec!["id".to_owned()],
        });
        let (plan, _) = plan(&create, Dialect::Postgres, 1, &[])?;
        assert!(plan.create.as_deref().is_some_and(|sql| {
            sql.starts_with("CREATE TABLE \"public\".\"people\" (\n  \"id\" bigint,")
        }));
        assert!(!plan.truncate);
        assert_eq!(plan.conflict, ConflictPolicy::Error);
        if let Some(definition) = create.create.as_mut() {
            definition.types[0] = "int; DROP TABLE x".to_owned();
        }
        assert!(super::plan(&create, Dialect::Postgres, 1, &[]).is_err());
        Ok(())
    }
}

/// Parser-classified batch template; values remain bound and the immutable plan controls every batch.
pub(crate) fn safety_statements(plan: &LoadPlan) -> Result<Vec<String>, IpcError> {
    let table = plan
        .dialect
        .qualified(&plan.table.schema, &plan.table.table)
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    if plan.mode == openrows_import::RunMode::Check {
        return Ok(vec![format!("SELECT COUNT(*) FROM {table}")]);
    }
    let columns = plan
        .columns
        .iter()
        .map(|column| plan.dialect.quote_ident(column))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| IpcError::invalid(error.to_string()))?
        .join(", ");
    let values = vec!["NULL"; plan.columns.len()].join(", ");
    let mut statements: Vec<String> = plan.create.iter().cloned().collect();
    if plan.truncate {
        statements.push(format!("DELETE FROM {table}"));
    }
    statements.push(format!("INSERT INTO {table} ({columns}) VALUES ({values})"));
    Ok(statements)
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    use openrows_core::safety::{Gate, SafetyPlan, SafetyPolicy};
    use openrows_types::safety::Safety;
    #[test]
    fn every_import_plan_is_checked_including_create_and_truncate() -> Result<(), IpcError> {
        let mut plan = LoadPlan {
            dialect: Dialect::Postgres,
            table: TableName {
                schema: "public".into(),
                table: "items".into(),
            },
            columns: vec!["id".into()],
            key: vec![],
            conflict: ConflictPolicy::Error,
            create: Some("CREATE TABLE items (id int)".into()),
            truncate: true,
            transaction: openrows_import::TransactionMode::Single,
            bad_rows: openrows_import::BadRows::Abort,
            mode: openrows_import::RunMode::Import,
        };
        let statements = safety_statements(&plan)?;
        assert_eq!(statements.len(), 3);
        let check = |mode, sql| {
            SafetyPlan::check(
                SafetyPolicy {
                    mode,
                    dialect: plan.dialect,
                },
                sql,
            )
        };
        assert_eq!(
            check(Safety::ReadOnly, statements.clone()).gate(),
            Gate::Block
        );
        assert!(check(Safety::AuthenticateWrites, statements).authentication_required);
        plan.mode = openrows_import::RunMode::Check;
        let dry = safety_statements(&plan)?;
        assert_eq!(dry.len(), 1);
        assert_eq!(check(Safety::ReadOnly, dry.clone()).gate(), Gate::Run);
        assert_eq!(check(Safety::ConfirmAll, dry).gate(), Gate::Confirm);
        Ok(())
    }
}
