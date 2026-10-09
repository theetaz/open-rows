// SPDX-License-Identifier: Apache-2.0
//! `table_detail` (Structure, Index, Triggers, DDL and inspector table info)
//! and `table_alter`: review, then commit, a typed structure change set as
//! DDL generated in Rust, in one transaction on a short-lived connection.
use super::alter_checks::{PRODUCTION, null_warnings};
use super::commands::owned_session;
use super::dto::TableTarget;
use super::edit_commands::{ensure_writable, session};
use super::schema_dto::{AlterReply, AlterRequest, TableDetailDto};
use super::schema_map::detail_dto;
use crate::connections::catalog::read_detail;
use crate::connections::catalog_cache::after_statements;
use crate::connections::drivers::{driver, profile_driver};
use crate::connections::profiles::profile_id;
use crate::connections::{Connections, IpcError, IpcErrorKind};
use openrows_core::safety::{SafetyPlan, SafetyPolicy};
use openrows_driver_api::schema::SchemaChange;
use openrows_driver_api::structure::{AlterPlan, AlterTable};
use openrows_driver_api::{Connection, Driver, catalog, editor};
use openrows_sql::ddl::view_body;
use openrows_sql::{Dialect, StatementKind, classify, split};
use openrows_store::profile::EnvKind;
use tauri::{AppHandle, State};

/// Most changes one review may hold.
const MAX_CHANGES: usize = 500;

/// Columns, constraints, indexes, triggers, sizes, comment and the rebuilt
/// definition of a relation, read on the metadata session.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_detail(
    state: State<'_, Connections>,
    target: TableTarget,
) -> Result<TableDetailDto, IpcError> {
    let read_only = state
        .profiles
        .get(profile_id(&target.profile_id)?)?
        .safety
        .read_only;
    let driver = profile_driver(&state, &target.profile_id)?;
    let detail = read_detail(
        &state,
        (&target.profile_id, &target.database),
        (target.schema.clone(), target.table.clone()),
    )
    .await?;
    let ddl = driver
        .table_ddl((&target.schema, &target.table), &detail)
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    let view_query = detail
        .info
        .as_ref()
        .and_then(|info| info.view_definition.as_deref())
        .map(|definition| view_body(definition, driver.dialect()));
    // Engines without a structure editor show their structure read-only.
    let editable = !read_only && driver.supports_alter();
    let mut dto = detail_dto(detail, ddl, !editable).ok_or_else(|| {
        IpcError::new(
            IpcErrorKind::NotFound,
            format!("{}.{} was not found", target.schema, target.table),
        )
    })?;
    dto.view_query = view_query;
    Ok(dto)
}

/// The connection's write safety for a review.
#[derive(Clone, Copy)]
struct Safety {
    /// Destructive changes need the typed table name.
    guarded: bool,
    /// A production connection: the review says so.
    production: bool,
}

/// Without a matching `confirmToken`, return the generated DDL to review;
/// with it (and the typed table name when asked), run it in one transaction.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_alter(
    app: AppHandle,
    state: State<'_, Connections>,
    request: AlterRequest,
) -> Result<AlterReply, IpcError> {
    owned_session(&state, &request.session_id, &request.target)?;
    let profile = state
        .profiles
        .get(profile_id(&request.target.profile_id)?)?;
    ensure_writable(profile.safety.read_only)?;
    if request.changes.is_empty() || request.changes.len() > MAX_CHANGES {
        return Err(IpcError::invalid(
            "Stage between 1 and 500 structure changes",
        ));
    }
    let production = profile.env_tag.kind == EnvKind::Prod;
    let safety = Safety {
        guarded: profile.safety.confirm_writes || production,
        production,
    };
    let driver = driver(profile.driver);
    let mut session = session(&state, &request.target).await?;
    let result = prepare_alter(session.as_mut(), driver, &request, safety).await;
    if matches!(result, Ok(AlterReply::Committed)) {
        let target = (
            request.target.profile_id.as_str(),
            request.target.database.as_str(),
        );
        after_statements(&app, target, [StatementKind::Ddl]);
    }
    let closed = session
        .close()
        .await
        .map_err(|error| IpcError::from(&error));
    // Keep the operation error (08007 especially) when closing also fails.
    if result.is_err() || matches!(result, Ok(AlterReply::Committed)) {
        return result;
    }
    closed?;
    result
}

async fn prepare_alter(
    session: &mut dyn Connection,
    driver: &dyn Driver,
    request: &AlterRequest,
    safety: Safety,
) -> Result<AlterReply, IpcError> {
    let target = &request.target;
    let detail = catalog(session)
        .map_err(|error| IpcError::from(&error))?
        .table_detail(&target.schema, &target.table)
        .await
        .map_err(|error| IpcError::from(&error))?;
    let changes: Vec<SchemaChange> = request.changes.iter().map(Into::into).collect();
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
    let dialect = driver.dialect();
    ensure_single_ddl(&plan.statements, dialect)?;
    let token = review_token(request, &plan.statements, dialect)?;
    let confirm_name = (safety.guarded && plan.destructive).then(|| target.table.clone());
    if request.confirm_token.as_deref() != Some(token.as_str()) {
        let mut plan = plan;
        plan.warnings
            .extend(null_warnings(session, dialect, target, &changes).await);
        if safety.production {
            plan.warnings.push(PRODUCTION.to_owned());
        }
        return Ok(prepared(token, plan, confirm_name));
    }
    if let Some(name) = confirm_name
        && request.confirm_name.as_deref() != Some(name.as_str())
    {
        return Err(IpcError::invalid(format!(
            "Type the table name {name:?} to confirm these destructive changes"
        )));
    }
    editor(session)
        .map_err(|error| IpcError::from(&error))?
        .commit_ddl(&plan.statements)
        .await
        .map_err(|error| IpcError::from(&error))?;
    Ok(AlterReply::Committed)
}

fn prepared(token: String, plan: AlterPlan, confirm_name: Option<String>) -> AlterReply {
    AlterReply::Prepared {
        token,
        statements: plan.statements,
        warnings: plan.warnings,
        destructive: plan.destructive,
        confirm_name,
    }
}

/// Defence in depth: every generated text must be exactly one DDL statement
/// (a default expression cannot smuggle in a second one). A `SQLite` table
/// rebuild also copies its rows (`INSERT INTO … SELECT`) and scopes the
/// legacy rename pragma around its final rename.
fn ensure_single_ddl(statements: &[String], dialect: Dialect) -> Result<(), IpcError> {
    for sql in statements {
        let single = split(sql, dialect).len() == 1;
        if !single || !generated_kind(sql, dialect) {
            return Err(IpcError::invalid(
                "A default expression must be a single SQL expression",
            ));
        }
    }
    Ok(())
}

fn generated_kind(sql: &str, dialect: Dialect) -> bool {
    match classify(sql, dialect).kind {
        StatementKind::Ddl => true,
        StatementKind::Write => sql.starts_with("INSERT INTO "),
        _ => {
            dialect == Dialect::Sqlite
                && matches!(
                    sql,
                    "PRAGMA legacy_alter_table = ON" | "PRAGMA legacy_alter_table = OFF"
                )
        }
    }
}

/// Binds a review to the session, the exact change set and its statements.
fn review_token(
    request: &AlterRequest,
    statements: &[String],
    dialect: Dialect,
) -> Result<String, IpcError> {
    let plan = SafetyPlan::check(
        SafetyPolicy {
            read_only: false,
            confirm_writes: true,
            dialect,
        },
        statements.to_vec(),
    );
    let scope = format!(
        "alter:{}:{}",
        request.session_id,
        serde_json::to_string(&request.changes)
            .map_err(|error| IpcError::invalid(error.to_string()))?
    );
    Ok(plan.confirmation_token(&scope))
}

#[cfg(test)]
mod tests {
    use super::super::schema_dto::{DefaultDto, SchemaChangeDto};
    use super::*;

    fn request(changes: Vec<SchemaChangeDto>) -> AlterRequest {
        AlterRequest {
            session_id: "session-a".into(),
            target: TableTarget {
                profile_id: "p".into(),
                database: "d".into(),
                schema: "public".into(),
                table: "t".into(),
            },
            changes,
            confirm_token: None,
            confirm_name: None,
        }
    }

    #[test]
    fn only_single_ddl_statements_pass() {
        assert!(
            ensure_single_ddl(
                &["ALTER TABLE \"t\" DROP COLUMN \"c\"".into()],
                Dialect::Postgres
            )
            .is_ok()
        );
        assert!(
            ensure_single_ddl(&["COMMENT ON TABLE \"t\" IS 'x'".into()], Dialect::Postgres).is_ok()
        );
        let smuggled = "ALTER TABLE \"t\" ALTER COLUMN \"c\" SET DEFAULT ('a'); DROP TABLE t; --)";
        assert!(ensure_single_ddl(&[smuggled.into()], Dialect::Postgres).is_err());
        assert!(ensure_single_ddl(&["DELETE FROM t".into()], Dialect::Postgres).is_err());
        let rebuild = [
            "INSERT INTO \"main\".\"n\" (\"a\") SELECT \"a\" FROM \"main\".\"t\"".to_owned(),
            "PRAGMA legacy_alter_table = ON".to_owned(),
        ];
        assert!(ensure_single_ddl(&rebuild, Dialect::Sqlite).is_ok());
        let pragma = ["PRAGMA foreign_keys = OFF".to_owned()];
        assert!(ensure_single_ddl(&pragma, Dialect::Sqlite).is_err());
        let legacy = ["PRAGMA legacy_alter_table = ON".to_owned()];
        assert!(ensure_single_ddl(&legacy, Dialect::Postgres).is_err());
    }

    #[test]
    fn review_tokens_bind_session_changes_and_statements() -> Result<(), IpcError> {
        let comment = |text: &str| SchemaChangeDto::TableComment {
            comment: Some(text.into()),
        };
        let mut first = request(vec![comment("a")]);
        let statements = vec!["COMMENT ON TABLE \"public\".\"t\" IS 'a'".to_owned()];
        let token = review_token(&first, &statements, Dialect::Postgres)?;
        assert_eq!(token, review_token(&first, &statements, Dialect::Postgres)?);
        let changed = request(vec![comment("b")]);
        assert_ne!(
            token,
            review_token(&changed, &statements, Dialect::Postgres)?
        );
        first.session_id = "session-b".into();
        assert_ne!(token, review_token(&first, &statements, Dialect::Postgres)?);
        Ok(())
    }

    #[test]
    fn change_sets_deserialize_from_camel_case() -> Result<(), serde_json::Error> {
        let changes: Vec<SchemaChangeDto> = serde_json::from_value(serde_json::json!([
            { "kind": "columnType", "column": "id", "dataType": "bigint" },
            { "kind": "columnDefault", "column": "at", "default": { "kind": "expression", "sql": "now()" } },
            { "kind": "addColumn", "name": "n", "dataType": "text", "nullable": true,
              "default": { "kind": "literal", "value": "x" }, "comment": null },
            { "kind": "createIndex", "name": null, "columns": ["n"], "unique": false, "method": "gin" },
            { "kind": "dropIndex", "name": "i" }
        ]))?;
        assert_eq!(
            changes[1],
            SchemaChangeDto::ColumnDefault {
                column: "at".into(),
                default: DefaultDto::Expression {
                    sql: "now()".into()
                },
            }
        );
        let model: Vec<SchemaChange> = changes.iter().map(Into::into).collect();
        assert!(
            matches!(&model[0], SchemaChange::ColumnType { data_type, .. } if data_type == "bigint")
        );
        Ok(())
    }
}
