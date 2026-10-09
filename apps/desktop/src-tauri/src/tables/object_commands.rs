// SPDX-License-Identifier: Apache-2.0
//! `object_change`: review, then run, one object lifecycle change
//! (FR-SCHEMA-10, 11, 12) generated in Rust; `routine_definition`: the
//! definition the routine editor starts from.
//!
//! Every change is first returned for review with a token bound to the
//! exact statements; it runs only when that token comes back (and, for a
//! destructive change on a confirm-writes or production connection, the
//! typed object name). Read-only connections refuse everything.
use super::alter_checks::PRODUCTION;
use super::dto::TableTarget;
use super::edit_commands::session;
use super::object_dto::{ObjectReply, ObjectRequest, RoutineTarget};
use crate::connections::catalog::{read_detail, read_objects, read_routine};
use crate::connections::catalog_cache::after_statements;
use crate::connections::drivers::driver;
use crate::connections::profiles::profile_id;
use crate::connections::{Connections, IpcError};
use openrows_core::safety::{SafetyPlan, SafetyPolicy};
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::objects::{ObjectChange, ObjectPlan, PlanRun, RoutineKind};
use openrows_driver_api::schema::TableDetail;
use openrows_driver_api::{Connection, DiscardSink, Driver, DriverError, editor};
use openrows_sql::{Dialect, StatementKind, classify, split};
use openrows_store::profile::EnvKind;
use tauri::{AppHandle, State};

/// Without a matching `confirmToken`, return the statements to review;
/// with it (and the typed name when asked), run them.
#[tauri::command]
#[specta::specta]
pub(crate) async fn object_change(
    app: AppHandle,
    state: State<'_, Connections>,
    request: ObjectRequest,
) -> Result<ObjectReply, IpcError> {
    let profile = state.profiles.get(profile_id(&request.profile_id)?)?;
    if profile.safety.read_only {
        return Err(IpcError::invalid(
            "This connection is read-only: structure changes are disabled",
        ));
    }
    let production = profile.env_tag.kind == EnvKind::Prod;
    let driver = driver(profile.driver);
    let change = ObjectChange::from(request.change.clone());
    let source = source_detail(&state, &request, &change).await?;
    check_target(&state, &request, &change).await?;
    let mut plan = driver
        .object_sql(&change, source.as_ref())
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    if production {
        plan.warnings.push(PRODUCTION.to_owned());
    }
    ensure_generated(&plan.statements, driver.dialect())?;
    let token = review_token(&request, &plan.statements, driver.dialect())?;
    let guarded = profile.safety.confirm_writes || production;
    let confirm_name =
        (guarded && plan.destructive).then(|| request.change.object_name().to_owned());
    if request.confirm_token.as_deref() != Some(token.as_str()) {
        return Ok(prepared(token, plan, confirm_name));
    }
    if let Some(name) = confirm_name
        && request.confirm_name.as_deref() != Some(name.as_str())
    {
        return Err(IpcError::invalid(format!(
            "Type {name:?} to confirm this change"
        )));
    }
    run_plan(&state, &request, driver, &plan).await?;
    after_statements(
        &app,
        (&request.profile_id, &request.database),
        [StatementKind::Ddl],
    );
    Ok(ObjectReply::Committed)
}

/// The `CREATE` statement of a function or procedure, read on the
/// database's metadata session; `None` when it is gone.
#[tauri::command]
#[specta::specta]
pub(crate) async fn routine_definition(
    state: State<'_, Connections>,
    target: RoutineTarget,
) -> Result<Option<String>, IpcError> {
    read_routine(
        &state,
        (&target.profile_id, &target.database),
        (target.schema, target.name),
        (target.routine.into(), target.arguments),
    )
    .await
}

fn prepared(token: String, plan: ObjectPlan, confirm_name: Option<String>) -> ObjectReply {
    ObjectReply::Prepared {
        token,
        statements: plan.statements,
        warnings: plan.warnings,
        destructive: plan.destructive,
        confirm_name,
        transactional: plan.run == PlanRun::Transaction,
    }
}

/// The detail of the table a duplicate copies.
async fn source_detail(
    state: &Connections,
    request: &ObjectRequest,
    change: &ObjectChange,
) -> Result<Option<TableDetail>, IpcError> {
    let ObjectChange::DuplicateTable { table, .. } = change else {
        return Ok(None);
    };
    read_detail(
        state,
        (&request.profile_id, &request.database),
        (table.schema.clone(), table.name.clone()),
    )
    .await
    .map(Some)
}

/// A dropped routine overload exists with exactly these arguments, and a
/// database being renamed or dropped is not open in the app.
async fn check_target(
    state: &Connections,
    request: &ObjectRequest,
    change: &ObjectChange,
) -> Result<(), IpcError> {
    match change {
        ObjectChange::DropRoutine {
            routine,
            kind,
            arguments,
        } => {
            let kind = match kind {
                RoutineKind::Function => ObjectKind::Function,
                RoutineKind::Procedure => ObjectKind::Procedure,
            };
            let objects = read_objects(
                state,
                (&request.profile_id, &request.database),
                routine.schema.clone(),
                vec![kind],
            )
            .await?;
            let found = objects.iter().any(|object| {
                object.name == routine.name
                    && (arguments.is_none() || object.arguments == *arguments)
            });
            if found {
                Ok(())
            } else {
                Err(IpcError::invalid(format!(
                    "{}.{} was not found; refresh the sidebar",
                    routine.schema, routine.name
                )))
            }
        }
        ObjectChange::RenameDatabase { name, .. } | ObjectChange::DropDatabase { name } => {
            let profile = profile_id(&request.profile_id)?;
            if state.owners.in_use(profile, name) {
                return Err(IpcError::invalid(format!(
                    "Close the database {name:?} in OpenRows first"
                )));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Each statement is exactly one DDL statement, or the row copy, `TRUNCATE`
/// or `SQLite` `DELETE` the plan generated.
fn ensure_generated(statements: &[String], dialect: Dialect) -> Result<(), IpcError> {
    for sql in statements {
        let single = split(sql, dialect).len() == 1;
        let allowed = match classify(sql, dialect).kind {
            StatementKind::Ddl => true,
            StatementKind::Write => ["INSERT INTO ", "TRUNCATE TABLE ", "DELETE FROM "]
                .iter()
                .any(|head| sql.starts_with(head)),
            _ => false,
        };
        if !single || !allowed {
            return Err(IpcError::invalid(
                "The definition must hold only the statements that create the object",
            ));
        }
    }
    Ok(())
}

/// Binds a review to the connection, database, change and statements.
fn review_token(
    request: &ObjectRequest,
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
        "object:{}:{}:{}",
        request.profile_id,
        request.database,
        serde_json::to_string(&request.change)
            .map_err(|error| IpcError::invalid(error.to_string()))?
    );
    Ok(plan.confirmation_token(&scope))
}

/// Run the plan on a short-lived connection: the object's database in one
/// transaction, or the home database one statement at a time.
async fn run_plan(
    state: &Connections,
    request: &ObjectRequest,
    driver: &dyn Driver,
    plan: &ObjectPlan,
) -> Result<(), IpcError> {
    let database = match plan.run {
        PlanRun::Transaction => request.database.clone(),
        PlanRun::HomeDatabase => driver.home_database().to_owned(),
    };
    let target = TableTarget {
        profile_id: request.profile_id.clone(),
        database,
        schema: String::new(),
        table: String::new(),
    };
    let mut connection = session(state, &target).await?;
    let result = execute(connection.as_mut(), plan).await;
    let closed = connection.close().await;
    // Keep the statement error (08007 especially) when closing also fails.
    result.map_err(|error| IpcError::from(&error))?;
    closed.map_err(|error| IpcError::from(&error))
}

async fn execute(connection: &mut dyn Connection, plan: &ObjectPlan) -> Result<(), DriverError> {
    match plan.run {
        PlanRun::Transaction => editor(connection)?.commit_ddl(&plan.statements).await,
        PlanRun::HomeDatabase => {
            for sql in &plan.statements {
                connection.execute(sql, &mut DiscardSink).await?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::object_dto::{ObjectChangeDto, ObjectNameDto, RelationChangeDto};
    use super::*;

    fn request(change: ObjectChangeDto) -> ObjectRequest {
        ObjectRequest {
            profile_id: "p".into(),
            database: "d".into(),
            change,
            confirm_token: None,
            confirm_name: None,
        }
    }

    fn drop_table(name: &str) -> ObjectChangeDto {
        ObjectChangeDto::Relation(RelationChangeDto::DropTable {
            target: ObjectNameDto {
                schema: "public".into(),
                name: name.into(),
            },
            cascade: false,
        })
    }

    #[test]
    fn only_generated_statements_pass() {
        let ok = [
            "DROP TABLE \"t\"".to_owned(),
            "INSERT INTO \"c\" (\"a\") SELECT \"a\" FROM \"t\"".to_owned(),
            "TRUNCATE TABLE \"t\"".to_owned(),
        ];
        assert!(ensure_generated(&ok, Dialect::Postgres).is_ok());
        assert!(ensure_generated(&["DELETE FROM \"main\".\"t\"".into()], Dialect::Sqlite).is_ok());
        for bad in [
            "SELECT 1",
            "UPDATE t SET a = 1",
            "DROP TABLE a; DROP TABLE b",
        ] {
            assert!(
                ensure_generated(&[bad.to_owned()], Dialect::Postgres).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn review_tokens_bind_the_change() -> Result<(), IpcError> {
        let statements = vec!["DROP TABLE \"public\".\"a\"".to_owned()];
        let first = review_token(&request(drop_table("a")), &statements, Dialect::Postgres)?;
        let second = review_token(&request(drop_table("b")), &statements, Dialect::Postgres)?;
        assert_ne!(first, second);
        assert_eq!(request(drop_table("a")).change.object_name(), "a");
        Ok(())
    }

    #[test]
    fn changes_deserialize_from_camel_case() -> Result<(), serde_json::Error> {
        let change: ObjectChangeDto = serde_json::from_value(serde_json::json!({
            "kind": "duplicateTable",
            "target": { "schema": "public", "name": "orders" },
            "name": "orders_copy",
            "withData": true
        }))?;
        assert!(matches!(
            ObjectChange::from(change),
            ObjectChange::DuplicateTable {
                with_data: true,
                ..
            }
        ));
        Ok(())
    }
}
