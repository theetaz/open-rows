// SPDX-License-Identifier: Apache-2.0
//! Attach and detach database files on a `SQLite` connection (M1-13).
//!
//! Attachments are saved in the profile, so every connection the app opens
//! afterwards (user, metadata and edit sessions) attaches them right after
//! opening. The commands save the profile first, then bring live state in
//! line: each open user session of the profile runs the same `ATTACH` or
//! `DETACH` (after any statement it is running), and a session that cannot
//! (for example inside an open transaction) is reported as a warning — the
//! saved change stays and the session follows it once reopened. The
//! profile's metadata sessions close and its cached catalog turns stale, so
//! the schema tree reads the new schema list.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri deserialises command arguments into owned values"
)]
use std::path::PathBuf;

use openrows_core::QueryEvent;
use openrows_core::script::{ScriptCanceller, ScriptOutcome, run_bound};
use openrows_schema::SchemaCache;
use openrows_sql::attach::{attach_sql, detach_sql};
use openrows_sql::select::{BoundSql, SelectError};
use openrows_store::profile::{
    AttachedFile, ConnectionProfile, FileParams, PasswordChange, ProfileId, ProfileService,
};
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};

use super::Connections;
use super::dto::ProfileDto;
use super::error::IpcError;
use super::profiles::profile_id;

/// The saved connection after an attach or detach, and what live sessions said.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachReply {
    /// The connection as saved.
    profile: ProfileDto,
    /// One message per open session that could not follow the change.
    warnings: Vec<String>,
}

/// Attach the existing database file at `path` as schema `alias` to the
/// saved `SQLite` connection and its open sessions. The file opens read-only
/// when the connection's main file does.
#[tauri::command]
#[specta::specta]
pub(crate) async fn connection_attach_database(
    app: AppHandle,
    profile_id: String,
    path: String,
    alias: String,
) -> Result<AttachReply, IpcError> {
    let id = self::profile_id(&profile_id)?;
    let state = app.state::<Connections>();
    let (saved, statement) = attach_saved(&state.profiles, id, &path, &alias)?;
    let warnings = follow_live(&app, (&profile_id, id), statement).await;
    Ok(AttachReply {
        profile: ProfileDto::from(&saved),
        warnings,
    })
}

/// Detach schema `alias` from the saved `SQLite` connection and its open sessions.
#[tauri::command]
#[specta::specta]
pub(crate) async fn connection_detach_database(
    app: AppHandle,
    profile_id: String,
    alias: String,
) -> Result<AttachReply, IpcError> {
    let id = self::profile_id(&profile_id)?;
    let state = app.state::<Connections>();
    let (saved, statement) = detach_saved(&state.profiles, id, &alias)?;
    let warnings = follow_live(&app, (&profile_id, id), statement).await;
    Ok(AttachReply {
        profile: ProfileDto::from(&saved),
        warnings,
    })
}

/// Save the attachment; returns the saved profile and the `ATTACH` to run
/// on open sessions.
fn attach_saved(
    service: &ProfileService,
    id: ProfileId,
    path: &str,
    alias: &str,
) -> Result<(ConnectionProfile, BoundSql), IpcError> {
    let mut profile = service.get(id)?;
    let file = sqlite_file(&mut profile)?;
    let attached = AttachedFile {
        alias: alias.trim().to_owned(),
        path: PathBuf::from(path.trim()),
    };
    let statement =
        attach_sql(&attached.alias, &attached.path, file.read_only).map_err(sql_error)?;
    let exists = attached.path.is_file();
    file.attach(attached)?;
    if !exists {
        return Err(IpcError::invalid(
            "Choose an existing database file to attach",
        ));
    }
    Ok((service.update(profile, PasswordChange::Keep)?, statement))
}

/// Remove the attachment; returns the saved profile and the `DETACH` to run
/// on open sessions.
fn detach_saved(
    service: &ProfileService,
    id: ProfileId,
    alias: &str,
) -> Result<(ConnectionProfile, BoundSql), IpcError> {
    let mut profile = service.get(id)?;
    let removed = sqlite_file(&mut profile)?.detach(alias.trim())?;
    let statement = detach_sql(&removed.alias).map_err(sql_error)?;
    Ok((service.update(profile, PasswordChange::Keep)?, statement))
}

fn sqlite_file(profile: &mut ConnectionProfile) -> Result<&mut FileParams, IpcError> {
    let refused = || IpcError::invalid("Only SQLite connections can attach databases");
    if !profile.driver.is_file() {
        return Err(refused());
    }
    profile.file.as_mut().ok_or_else(refused)
}

fn sql_error(error: SelectError) -> IpcError {
    IpcError::invalid(error.to_string())
}

/// Run `statement` on every open user session of the profile, then close
/// its metadata sessions and mark its cached catalog stale; returns a
/// warning per session that failed.
async fn follow_live(
    app: &AppHandle,
    (key, id): (&str, ProfileId),
    statement: BoundSql,
) -> Vec<String> {
    let state = app.state::<Connections>();
    let mut warnings = Vec::new();
    for session in state.owners.sessions(id) {
        let mut failure = None;
        let canceller = ScriptCanceller::default();
        let outcome = run_bound(
            &state.sessions,
            session,
            statement.clone(),
            &canceller,
            |event| {
                if let QueryEvent::Failed { error } = event.event {
                    failure = Some(error.message);
                }
            },
        )
        .await;
        if outcome != ScriptOutcome::Completed {
            warnings.push(session_warning(failure.as_deref()));
        }
    }
    state.metadata.close(key).await;
    app.state::<SchemaCache>().invalidate(key);
    warnings
}

fn session_warning(reason: Option<&str>) -> String {
    format!(
        "An open tab could not follow the change ({}); it applies once the connection is reopened.",
        reason.unwrap_or("cancelled")
    )
}

#[cfg(test)]
#[path = "attach_tests.rs"]
mod tests;
