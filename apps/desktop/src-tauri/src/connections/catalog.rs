// SPDX-License-Identifier: Apache-2.0
//! Schema-tree commands (D-06): databases, schemas, objects and the name
//! index, read on the connection's per-database metadata sessions (closed by
//! `session_close` and `connection_close`).
use openrows_driver_api::catalog::{ColumnSummary, ObjectKind, TableStructure};
use tauri::State;

use super::Connections;
use super::catalog_dto::{DatabaseDto, ObjectDto, ObjectKindDto, SchemaDto};
use super::error::IpcError;
use super::pg::{self, Target};
use super::profiles::profile_id;

/// Resolve a saved connection's target, reading its password from the keychain.
pub(crate) fn profile_target(state: &Connections, id: &str) -> Result<Target, IpcError> {
    let profile = state.profiles.get(profile_id(id)?)?;
    let password = state.profiles.password(&profile)?;
    pg::target(&profile.params, profile.tls.mode, password.as_ref())
}

/// Databases on the server, read through `database`'s metadata session.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_databases(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
) -> Result<Vec<DatabaseDto>, IpcError> {
    let databases = state
        .metadata
        .read(
            (&profile_id, &database),
            || profile_target(&state, &profile_id),
            |session| Box::pin(session.list_databases()),
        )
        .await?;
    Ok(databases.into_iter().map(DatabaseDto::from).collect())
}

/// Schemas of `database`, user schemas first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_schemas(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
) -> Result<Vec<SchemaDto>, IpcError> {
    let schemas = state
        .metadata
        .read(
            (&profile_id, &database),
            || profile_target(&state, &profile_id),
            |session| Box::pin(session.list_schemas()),
        )
        .await?;
    Ok(schemas.into_iter().map(SchemaDto::from).collect())
}

/// Objects of `kinds` in `database`.`schema`, ordered by name.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_objects(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
    schema: String,
    kinds: Vec<ObjectKindDto>,
) -> Result<Vec<ObjectDto>, IpcError> {
    let kinds: Vec<ObjectKind> = kinds.into_iter().map(ObjectKind::from).collect();
    let objects = state
        .metadata
        .read(
            (&profile_id, &database),
            || profile_target(&state, &profile_id),
            move |session| Box::pin(async move { session.list_objects(&schema, &kinds).await }),
        )
        .await?;
    Ok(objects.into_iter().map(ObjectDto::from).collect())
}

/// Bounded name index of `database` (tables, views, sequences, routines)
/// for quick open.
#[tauri::command]
#[specta::specta]
pub(crate) async fn catalog_search(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
) -> Result<Vec<ObjectDto>, IpcError> {
    let objects = state
        .metadata
        .read(
            (&profile_id, &database),
            || profile_target(&state, &profile_id),
            |session| Box::pin(session.search_index()),
        )
        .await?;
    Ok(objects.into_iter().map(ObjectDto::from).collect())
}

/// A relation's columns, indexes and row estimate on `database`'s metadata
/// session (the D-08 table view's Structure and Indexes tabs).
pub(crate) async fn read_structure(
    state: &Connections,
    (profile_id, database): (&str, &str),
    (schema, name): (String, String),
) -> Result<TableStructure, IpcError> {
    state
        .metadata
        .read(
            (profile_id, database),
            || profile_target(state, profile_id),
            move |session| Box::pin(async move { session.table_structure(&schema, &name).await }),
        )
        .await
}

/// A relation's catalog columns on `database`'s metadata session; table
/// browsing validates every requested identifier against them.
pub(crate) async fn read_columns(
    state: &Connections,
    (profile_id, database): (&str, &str),
    (schema, name): (String, String),
) -> Result<Vec<ColumnSummary>, IpcError> {
    state
        .metadata
        .read(
            (profile_id, database),
            || profile_target(state, profile_id),
            move |session| Box::pin(async move { session.table_columns(&schema, &name).await }),
        )
        .await
}
