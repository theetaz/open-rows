// SPDX-License-Identifier: Apache-2.0
//! Review and commit table edits on a dedicated, short-lived connection of
//! the table's engine, with SQL in that engine's dialect.
use super::commands::owned_session;
use super::dto::TableTarget;
use super::edit_dto::{EditInfoDto, EditReply, EditRequest, EditStatementDto};
use crate::connections::{Connections, IpcError};
use crate::connections::{profiles::profile_id, route, target};
use crate::queries::history::{CommitRecord, now_ms, record_commit};
use openrows_core::safety::{SafetyPlan, SafetyPolicy};
use openrows_driver_api::{Connection, DriverError, RowEditInfo, catalog, editor};
use openrows_sql::Dialect;
use openrows_sql::edit::{Edit, EditTable, edit_sql};
use openrows_sql::select::{BoundSql, TableRef};
use openrows_store::LocalStore;
use openrows_store::profile::EnvKind;
use std::time::Instant;
use tauri::Manager;
use tauri::State;

/// A dedicated connection to the target's database; closing it rolls back
/// anything still open.
pub(crate) async fn session(
    state: &Connections,
    target: &TableTarget,
) -> Result<Box<dyn Connection>, IpcError> {
    let destination = route::profile_target(state, &target.profile_id)
        .await?
        .with_database(&target.database)
        .with_application_suffix("table edits");
    target::connect(&destination)
        .await
        .map(|connected| connected.connection)
        .map_err(|error| IpcError::from(&error))
}

/// The table's key, generated columns and catalog column names, then the
/// hidden rowid of a keyless table: guarded like a column, never written
/// (`info.protected` lists it).
pub(crate) async fn edit_shape(
    connection: &mut dyn Connection,
    (schema, table): (&str, &str),
) -> Result<(RowEditInfo, Vec<String>), DriverError> {
    let info = editor(connection)?.row_edit_info(schema, table).await?;
    let columns = catalog(connection)?.table_columns(schema, table).await?;
    let names = columns
        .into_iter()
        .map(|column| column.name)
        .chain(info.row_id.clone())
        .collect();
    Ok((info, names))
}
fn writable(state: &Connections, target: &TableTarget) -> Result<(), IpcError> {
    let profile = state.profiles.get(profile_id(&target.profile_id)?)?;
    ensure_writable(profile.safety.read_only())
}
pub(crate) fn ensure_writable(read_only: bool) -> Result<(), IpcError> {
    if read_only {
        return Err(IpcError::invalid(
            "This connection is read-only: table edits are disabled",
        ));
    }
    Ok(())
}
/// Catalog key, generated columns and the profile's safety switches, without modifying data.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_edit_info(
    state: State<'_, Connections>,
    target: TableTarget,
) -> Result<EditInfoDto, IpcError> {
    let profile = state.profiles.get(profile_id(&target.profile_id)?)?;
    let safety = (
        profile.safety.read_only(),
        profile.safety.confirm_writes(),
        profile.env_tag.kind == EnvKind::Prod,
    );
    let mut session = session(&state, &target).await?;
    let result = async {
        let info = editor(session.as_mut())?
            .row_edit_info(&target.schema, &target.table)
            .await?;
        let columns = catalog(session.as_mut())?
            .table_columns(&target.schema, &target.table)
            .await?;
        Ok::<_, DriverError>(EditInfoDto {
            key: info.key,
            protected: info.protected,
            row_id: info.row_id,
            columns: columns.into_iter().map(Into::into).collect(),
            read_only: safety.0,
            confirm_writes: safety.1,
            production: safety.2,
        })
    }
    .await
    .map_err(|error| IpcError::from(&error));
    let closed = session
        .close()
        .await
        .map_err(|error| IpcError::from(&error));
    closed?;
    result
}
/// Prepare exact SQL/binds, then commit only when that preview's token is supplied.
#[tracing::instrument(name = "table.commit", skip_all, fields(changes = request.changes.len()))]
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_edit(
    app: tauri::AppHandle,
    state: State<'_, Connections>,
    local: State<'_, LocalStore>,
    request: EditRequest,
) -> Result<EditReply, IpcError> {
    owned_session(&state, &request.session_id, &request.target)?;
    writable(&state, &request.target)?;
    if request.changes.is_empty() || request.changes.len() > 1000 {
        return Err(IpcError::invalid(
            "Stage between 1 and 1,000 changes per commit",
        ));
    }
    let dialect =
        crate::connections::drivers::profile_driver(&state, &request.target.profile_id)?.dialect();
    let profile = state
        .profiles
        .get(profile_id(&request.target.profile_id)?)?;
    let mut session = session(&state, &request.target).await?;
    let history = Some((&*local, &profile));
    let result = prepare_commit(session.as_mut(), &request, dialect, (history, &app)).await;
    log_commit(&result);
    // Closing rolls back transactions still open; a lost COMMIT reply remains uncertain.
    let closed = session
        .close()
        .await
        .map_err(|error| IpcError::from(&error));
    // Preserve the operation error, especially SQLSTATE 08007, if closing also fails.
    if result.is_err() || matches!(result, Ok(EditReply::Committed)) {
        return result;
    }
    closed?;
    result
}
/// Log a commit's outcome as its kind only: messages can echo staged values.
fn log_commit(result: &Result<EditReply, IpcError>) {
    match result {
        Ok(EditReply::Committed) => tracing::info!("staged edits committed"),
        Ok(EditReply::Prepared { .. }) => tracing::debug!("staged edits previewed"),
        Err(error) => tracing::warn!(kind = ?error.kind, "staged edits not committed"),
    }
}

/// Plan the edits and commit them once the preview token is confirmed; a
/// committed or failed batch is written to `history` when it is given.
async fn prepare_commit(
    session: &mut dyn Connection,
    request: &EditRequest,
    dialect: Dialect,
    (history, app): (
        Option<(&LocalStore, &openrows_store::profile::ConnectionProfile)>,
        &tauri::AppHandle,
    ),
) -> Result<EditReply, IpcError> {
    let target = &request.target;
    let (info, columns) = edit_shape(session, (&target.schema, &target.table))
        .await
        .map_err(|error| IpcError::from(&error))?;
    let table = EditTable {
        table: TableRef {
            schema: target.schema.clone(),
            name: target.table.clone(),
        },
        columns,
        key: info.key,
        protected: info.protected,
    };
    let edits = request
        .changes
        .iter()
        .map(|change| {
            edit_sql(dialect, &table, &Edit::from(change))
                .map_err(|error| IpcError::invalid(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let token = preview_token(request, &edits, dialect)?;
    if request.confirm_token.as_deref() != Some(token.as_str()) {
        return Ok(EditReply::Prepared {
            token,
            statements: edits
                .into_iter()
                .map(|edit| EditStatementDto {
                    sql: edit.sql,
                    params: edit.params,
                })
                .collect(),
        });
    }
    if let Some((_, profile)) = history {
        crate::security::authenticate_plan(
            &edit_safety(crate::queries::commands::policy(profile), &edits),
            &format!("{} / {}", profile.name, request.target.database),
        )
        .await?;
    }
    app.state::<crate::security::Security>()
        .admit("table_edit")?;
    let started = (now_ms(), Instant::now());
    let outcome = editor(session)
        .map_err(|error| IpcError::from(&error))?
        .commit_edits(&edits)
        .await;
    if let Some(local) = history.filter(|(_, profile)| profile.record_history) {
        record_batch(local, request, &edits, (started, &outcome));
    }
    outcome.map_err(|error| IpcError::from(&error))?;
    Ok(EditReply::Committed)
}

/// One history entry per statement of the batch, with the batch's outcome.
fn record_batch(
    (local, profile): (&LocalStore, &openrows_store::profile::ConnectionProfile),
    request: &EditRequest,
    edits: &[BoundSql],
    ((started_at_ms, started), outcome): ((i64, Instant), &Result<(), DriverError>),
) {
    let statements: Vec<String> = edits.iter().map(|edit| edit.sql.clone()).collect();
    let failure = outcome
        .as_ref()
        .err()
        .map(|error| IpcError::from(error).message);
    record_commit(
        local,
        &CommitRecord {
            profile_id: &request.target.profile_id,
            database: &request.target.database,
            started_at_ms,
            duration: started.elapsed(),
            statements: &statements,
            error: failure.as_deref(),
            privacy: (
                crate::queries::history_privacy::redacts(profile),
                crate::queries::commands::policy(profile).dialect,
            ),
        },
    );
}

fn edit_safety(policy: SafetyPolicy, edits: &[BoundSql]) -> SafetyPlan {
    SafetyPlan::check(policy, edits.iter().map(|edit| edit.sql.clone()).collect())
}

fn preview_token(
    request: &EditRequest,
    edits: &[BoundSql],
    dialect: Dialect,
) -> Result<String, IpcError> {
    let plan = SafetyPlan::check(
        SafetyPolicy {
            mode: openrows_types::safety::Safety::ConfirmWrites,
            dialect,
        },
        edits.iter().map(|edit| edit.sql.clone()).collect(),
    );
    let scope = format!(
        "{}:{}",
        request.session_id,
        serde_json::to_string(&request.changes)
            .map_err(|error| IpcError::invalid(error.to_string()))?
    );
    Ok(plan.confirmation_token(&scope))
}
#[cfg(test)]
mod tests {
    use super::super::edit_dto::{CellValueDto, EditKindDto, RowEditDto};
    use super::*;
    use openrows_core::safety::Gate;
    use std::collections::BTreeMap;
    #[test]
    fn edit_apply_uses_the_saved_policy_for_generated_sql() -> Result<(), IpcError> {
        use openrows_types::safety::Safety;
        let edit = edit_sql(
            Dialect::Postgres,
            &users(),
            &Edit::from(&staged(EditKindDto::Update)),
        )
        .map_err(|error| IpcError::invalid(error.to_string()))?;
        for (mode, gate) in [
            (Safety::Unrestricted, Gate::Run),
            (Safety::ConfirmAll, Gate::Confirm),
            (Safety::ConfirmWrites, Gate::Confirm),
            (Safety::AuthenticateAll, Gate::Confirm),
            (Safety::AuthenticateWrites, Gate::Confirm),
            (Safety::ReadOnly, Gate::Block),
        ] {
            let checked = edit_safety(
                SafetyPolicy {
                    mode,
                    dialect: Dialect::Postgres,
                },
                std::slice::from_ref(&edit),
            );
            assert_eq!(checked.gate(), gate);
            assert_eq!(checked.authentication_required, mode.authenticates());
        }
        Ok(())
    }

    #[test]
    fn read_only_refuses_table_writes() {
        assert!(ensure_writable(true).is_err());
        assert!(ensure_writable(false).is_ok());
    }
    #[test]
    fn confirmation_is_bound_to_values_and_session() -> Result<(), IpcError> {
        let change = RowEditDto {
            kind: EditKindDto::Update,
            original: BTreeMap::from([("id".into(), Some("1".into()))]),
            values: BTreeMap::from([(
                "name".into(),
                CellValueDto::Text {
                    value: "first".into(),
                },
            )]),
        };
        let mut request = EditRequest {
            session_id: "session-a".into(),
            target: TableTarget {
                profile_id: "p".into(),
                database: "d".into(),
                schema: "public".into(),
                table: "t".into(),
            },
            changes: vec![change],
            confirm_token: None,
        };
        let sql = vec![BoundSql {
            sql: "UPDATE t SET name = $1 WHERE id = $2".into(),
            params: vec![Some("first".into()), Some("1".into())],
        }];
        let first = preview_token(&request, &sql, Dialect::Postgres)?;
        request.changes[0].values.insert(
            "name".into(),
            CellValueDto::Text {
                value: "second".into(),
            },
        );
        let second = preview_token(&request, &sql, Dialect::Postgres)?;
        assert_ne!(first, second);
        request.changes[0]
            .values
            .insert("name".into(), CellValueDto::Default);
        assert_ne!(second, preview_token(&request, &sql, Dialect::Postgres)?);
        request.session_id = "session-b".into();
        assert_ne!(first, preview_token(&request, &sql, Dialect::Postgres)?);
        Ok(())
    }
    fn users() -> EditTable {
        EditTable {
            table: TableRef {
                schema: "public".into(),
                name: "users".into(),
            },
            columns: vec!["id".into(), "name".into()],
            key: vec!["id".into()],
            protected: vec![],
        }
    }
    fn staged(kind: EditKindDto) -> RowEditDto {
        let original = BTreeMap::from([
            ("id".into(), Some("1".into())),
            ("name".into(), Some("old".into())),
        ]);
        let values = BTreeMap::from([("name".into(), CellValueDto::Null)]);
        match kind {
            EditKindDto::Update => RowEditDto {
                kind,
                original,
                values,
            },
            EditKindDto::Delete => RowEditDto {
                kind,
                original,
                values: BTreeMap::new(),
            },
            EditKindDto::Insert => RowEditDto {
                kind,
                original: BTreeMap::new(),
                values,
            },
        }
    }
    /// The editor can never produce an unbounded DELETE/UPDATE: every statement
    /// carries the full original-row guard and a read-only profile blocks all of them.
    #[test]
    fn generated_statements_are_bounded_and_blocked_on_read_only() -> Result<(), IpcError> {
        let sql = [
            EditKindDto::Update,
            EditKindDto::Delete,
            EditKindDto::Insert,
        ]
        .into_iter()
        .map(|kind| {
            edit_sql(Dialect::Postgres, &users(), &Edit::from(&staged(kind)))
                .map(|bound| bound.sql)
                .map_err(|error| IpcError::invalid(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
        assert!(sql[0].contains(" WHERE \"id\"::text IS NOT DISTINCT FROM $"));
        assert!(sql[1].contains(" WHERE \"id\"::text IS NOT DISTINCT FROM $"));
        let plan = SafetyPlan::check(
            SafetyPolicy {
                mode: openrows_types::safety::Safety::Unrestricted,
                dialect: Dialect::Postgres,
            },
            sql.clone(),
        );
        assert!(
            plan.statements
                .iter()
                .all(|statement| !statement.class.dangerous),
            "bounded DML is never classified dangerous"
        );
        assert_eq!(plan.gate(), Gate::Run);
        let blocked = SafetyPlan::check(
            SafetyPolicy {
                mode: openrows_types::safety::Safety::ReadOnly,
                dialect: Dialect::Postgres,
            },
            sql,
        );
        assert_eq!(blocked.gate(), Gate::Block);
        assert!(ensure_writable(true).is_err());
        Ok(())
    }
    /// A keyless `SQLite` table's rowid guards every UPDATE/DELETE like a
    /// column, the statements stay bounded, and it can never be written.
    #[test]
    fn rowid_edits_are_bounded_and_never_write_the_rowid() -> Result<(), IpcError> {
        let mut table = users();
        table.columns = vec!["name".into(), "rowid".into()];
        table.key = vec!["rowid".into()];
        table.protected = vec!["rowid".into()];
        let mut change = staged(EditKindDto::Delete);
        change.original.remove("id");
        change.original.insert("rowid".into(), Some("7".into()));
        let delete = edit_sql(Dialect::Sqlite, &table, &Edit::from(&change))
            .map_err(|error| IpcError::invalid(error.to_string()))?;
        assert!(
            delete
                .sql
                .ends_with("CAST(\"rowid\" AS TEXT) COLLATE BINARY IS ?2")
        );
        let policy = SafetyPolicy {
            mode: openrows_types::safety::Safety::Unrestricted,
            dialect: Dialect::Sqlite,
        };
        let plan = SafetyPlan::check(policy, vec![delete.sql]);
        assert!(plan.statements.iter().all(|item| !item.class.dangerous));
        change.kind = EditKindDto::Update;
        change.values =
            BTreeMap::from([("rowid".into(), CellValueDto::Text { value: "8".into() })]);
        assert!(edit_sql(Dialect::Sqlite, &table, &Edit::from(&change)).is_err());
        Ok(())
    }
    /// A commit needs the preview token, so the review sheet is shown on every commit.
    #[test]
    fn a_delete_without_a_key_is_refused_before_any_sql_exists() {
        let mut table = users();
        table.key.clear();
        assert!(
            edit_sql(
                Dialect::Postgres,
                &table,
                &Edit::from(&staged(EditKindDto::Delete))
            )
            .is_err()
        );
        let mut change = staged(EditKindDto::Delete);
        change.original.remove("id");
        assert!(edit_sql(Dialect::Postgres, &users(), &Edit::from(&change)).is_err());
    }
}
