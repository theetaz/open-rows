// SPDX-License-Identifier: Apache-2.0
//! Review and commit table edits on a dedicated, short-lived `PostgreSQL` connection.
use super::commands::owned_session;
use super::dto::TableTarget;
use super::edit_dto::{EditInfoDto, EditReply, EditRequest, EditStatementDto};
use crate::connections::{Connections, IpcError};
use crate::connections::{catalog, pg, profiles::profile_id};
use openrows_core::safety::{SafetyPlan, SafetyPolicy};
use openrows_driver_api::Session;
use openrows_driver_postgres::{BoundEdit, PgSession};
use openrows_sql::edit::{Edit, EditTable, edit_sql};
use openrows_sql::select::TableRef;
use std::collections::BTreeMap;
use tauri::State;

pub(crate) async fn session(
    state: &Connections,
    target: &TableTarget,
) -> Result<PgSession, IpcError> {
    let destination = catalog::profile_target(state, &target.profile_id)?
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
/// Catalog key and generated columns, without modifying data.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_edit_info(
    state: State<'_, Connections>,
    target: TableTarget,
) -> Result<EditInfoDto, IpcError> {
    let read_only = state
        .profiles
        .get(profile_id(&target.profile_id)?)?
        .safety
        .read_only;
    let mut session = session(&state, &target).await?;
    let result = async {
        let info = session.row_edit_info(&target.schema, &target.table).await?;
        let columns = session.table_columns(&target.schema, &target.table).await?;
        Ok::<_, openrows_driver_api::DriverError>(EditInfoDto {
            key: info.key,
            protected: info.protected,
            columns: columns.into_iter().map(Into::into).collect(),
            read_only,
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
/// Read exact original values for an identified row; reject unknown/missing keys.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_edit_row(
    state: State<'_, Connections>,
    target: TableTarget,
    keys: BTreeMap<String, String>,
) -> Result<BTreeMap<String, Option<String>>, IpcError> {
    let mut session = session(&state, &target).await?;
    let result = async {
        let info = session
            .row_edit_info(&target.schema, &target.table)
            .await
            .map_err(|error| IpcError::from(&error))?;
        if info.key.is_empty()
            || info.key.len() != keys.len()
            || info.key.iter().any(|name| !keys.contains_key(name))
        {
            return Err(IpcError::invalid(
                "This row needs a complete, supported unique key",
            ));
        }
        session
            .edit_snapshot(
                (&target.schema, &target.table),
                &keys.into_iter().collect::<Vec<_>>(),
            )
            .await
            .map(|row| row.into_iter().collect())
            .map_err(|error| IpcError::from(&error))
    }
    .await;
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
}
