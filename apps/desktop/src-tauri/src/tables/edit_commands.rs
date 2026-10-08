// SPDX-License-Identifier: Apache-2.0
//! Review and commit table edits on a dedicated, short-lived `PostgreSQL` connection.
use super::commands::owned_session;
use super::dto::TableTarget;
use super::edit_dto::{EditInfoDto, EditReply, EditRequest, EditStatementDto};
use crate::connections::{Connections, IpcError};
use crate::connections::{pg, profiles::profile_id, route};
use openrows_core::safety::{SafetyPlan, SafetyPolicy};
use openrows_driver_api::Session;
use openrows_driver_postgres::{BoundEdit, PgSession};
use openrows_sql::edit::{Edit, EditTable, edit_sql};
use openrows_sql::select::TableRef;
use openrows_store::profile::EnvKind;
use tauri::State;

pub(crate) async fn session(
    state: &Connections,
    target: &TableTarget,
) -> Result<PgSession, IpcError> {
    let destination = route::profile_target(state, &target.profile_id)
        .await?
        .with_database(&target.database)
        .with_application_suffix("table edits");
    pg::connect(&destination)
        .await
        .map(|connected| connected.session)
        .map_err(|error| IpcError::from(&error))
}
fn writable(state: &Connections, target: &TableTarget) -> Result<(), IpcError> {
    let profile = state.profiles.get(profile_id(&target.profile_id)?)?;
    ensure_writable(profile.safety.read_only)
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
        profile.safety.read_only,
        profile.safety.confirm_writes,
        profile.env_tag.kind == EnvKind::Prod,
    );
    let mut session = session(&state, &target).await?;
    let result = async {
        let info = session.row_edit_info(&target.schema, &target.table).await?;
        let columns = session.table_columns(&target.schema, &target.table).await?;
        Ok::<_, openrows_driver_api::DriverError>(EditInfoDto {
            key: info.key,
            protected: info.protected,
            columns: columns.into_iter().map(Into::into).collect(),
            read_only: safety.0,
            confirm_writes: safety.1,
            production: safety.2,
        })
    }
    .await
    .map_err(|error| IpcError::from(&error));
    let closed = Box::new(session)
        .close()
        .await
        .map_err(|error| IpcError::from(&error));
    closed?;
    result
}
/// Prepare exact SQL/binds, then commit only when that preview's token is supplied.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_edit(
    state: State<'_, Connections>,
    request: EditRequest,
) -> Result<EditReply, IpcError> {
    owned_session(&state, &request.session_id, &request.target)?;
    writable(&state, &request.target)?;
    if request.changes.is_empty() || request.changes.len() > 1000 {
        return Err(IpcError::invalid(
            "Stage between 1 and 1,000 changes per commit",
        ));
    }
    let mut session = session(&state, &request.target).await?;
    let result = prepare_commit(&mut session, &request).await;
    // Closing rolls back transactions still open; a lost COMMIT reply remains uncertain.
    let closed = Box::new(session)
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
async fn prepare_commit(
    session: &mut PgSession,
    request: &EditRequest,
) -> Result<EditReply, IpcError> {
    let info = session
        .row_edit_info(&request.target.schema, &request.target.table)
        .await
        .map_err(|error| IpcError::from(&error))?;
    let columns = session
        .table_columns(&request.target.schema, &request.target.table)
        .await
        .map_err(|error| IpcError::from(&error))?;
    let table = EditTable {
        table: TableRef {
            schema: request.target.schema.clone(),
            name: request.target.table.clone(),
        },
        columns: columns.into_iter().map(|column| column.name).collect(),
        key: info.key,
        protected: info.protected,
    };
    let edits = request
        .changes
        .iter()
        .map(|change| {
            edit_sql(&table, &Edit::from(change))
                .map_err(|error| IpcError::invalid(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let token = preview_token(request, &edits)?;
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
    let edits = edits
        .into_iter()
        .map(|edit| BoundEdit {
            sql: edit.sql,
            params: edit.params,
        })
        .collect::<Vec<_>>();
    session
        .commit_edits(&edits)
        .await
        .map_err(|error| IpcError::from(&error))?;
    Ok(EditReply::Committed)
}

fn preview_token(
    request: &EditRequest,
    edits: &[openrows_sql::select::BoundSql],
) -> Result<String, IpcError> {
    let plan = SafetyPlan::check(
        SafetyPolicy {
            read_only: false,
            confirm_writes: true,
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
        let sql = vec![openrows_sql::select::BoundSql {
            sql: "UPDATE t SET name = $1 WHERE id = $2".into(),
            params: vec![Some("first".into()), Some("1".into())],
        }];
        let first = preview_token(&request, &sql)?;
        request.changes[0].values.insert(
            "name".into(),
            CellValueDto::Text {
                value: "second".into(),
            },
        );
        let second = preview_token(&request, &sql)?;
        assert_ne!(first, second);
        request.changes[0]
            .values
            .insert("name".into(), CellValueDto::Default);
        assert_ne!(second, preview_token(&request, &sql)?);
        request.session_id = "session-b".into();
        assert_ne!(first, preview_token(&request, &sql)?);
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
            edit_sql(&users(), &Edit::from(&staged(kind)))
                .map(|bound| bound.sql)
                .map_err(|error| IpcError::invalid(error.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
        assert!(sql[0].contains(" WHERE \"id\"::text IS NOT DISTINCT FROM $"));
        assert!(sql[1].contains(" WHERE \"id\"::text IS NOT DISTINCT FROM $"));
        let plan = SafetyPlan::check(
            SafetyPolicy {
                read_only: false,
                confirm_writes: false,
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
                read_only: true,
                confirm_writes: false,
            },
            sql,
        );
        assert_eq!(blocked.gate(), Gate::Block);
        assert!(ensure_writable(true).is_err());
        Ok(())
    }
    /// A commit needs the preview token, so the review sheet is shown on every commit.
    #[test]
    fn a_delete_without_a_key_is_refused_before_any_sql_exists() {
        let mut table = users();
        table.key.clear();
        assert!(edit_sql(&table, &Edit::from(&staged(EditKindDto::Delete))).is_err());
        let mut change = staged(EditKindDto::Delete);
        change.original.remove("id");
        assert!(edit_sql(&users(), &Edit::from(&change)).is_err());
    }
}
