// SPDX-License-Identifier: Apache-2.0
//! Process list commands (FR-ADMIN-01): list a server's sessions and cancel
//! or terminate one, on the database's metadata connection so they never
//! queue behind the user's statements. Signalling is a write: refused on
//! read-only connections, and the connection the list itself runs on is
//! only signalled when the user confirmed exactly that.
use openrows_driver_api::{Admin, BoxFuture, DriverError, Feature, ProcessId, admin};
use openrows_store::profile::ConnectionProfile;
use tauri::State;

use super::Connections;
use super::drivers::driver;
use super::error::{IpcError, IpcErrorKind};
use super::processes_dto::{ProcessDto, ProcessSignalRequest};
use super::profiles::profile_id;
use super::route::profile_target;
use super::target::APPLICATION_NAME;

/// What a signal request came to.
enum Signalled {
    Done,
    /// The process is the metadata connection itself and `allow_own` was off.
    Own,
}

/// The saved profile, refused when its driver has no process list.
fn admin_profile(state: &Connections, id: &str) -> Result<ConnectionProfile, IpcError> {
    let profile = state.profiles.get(profile_id(id)?)?;
    let engine = driver(profile.driver);
    if !engine.capabilities().supports(Feature::ProcessList) {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            format!("{} has no server process list", engine.product()),
        ));
    }
    Ok(profile)
}

/// Run `act` on the admin facet of `database`'s metadata session.
async fn on_admin<T>(
    state: &Connections,
    (profile_id, database): (&str, &str),
    act: impl for<'s> FnOnce(&'s mut dyn Admin) -> BoxFuture<'s, Result<T, DriverError>>,
) -> Result<T, IpcError> {
    state
        .metadata
        .read(
            (profile_id, database),
            || profile_target(state, profile_id),
            move |connection| match admin(connection) {
                Ok(facet) => act(facet),
                Err(error) => Box::pin(async move { Err(error) }),
            },
        )
        .await
}

/// Client sessions on the server of `database`'s connection, the longest
/// running first, read on its metadata connection.
#[tauri::command]
#[specta::specta]
pub(crate) async fn processes_list(
    state: State<'_, Connections>,
    profile_id: String,
    database: String,
) -> Result<Vec<ProcessDto>, IpcError> {
    let profile = admin_profile(&state, &profile_id)?;
    let application = profile
        .params
        .application_name
        .unwrap_or_else(|| APPLICATION_NAME.to_owned());
    if let Some(processes) =
        super::monitor::processes::shared(&state, (&profile_id, &database), &application)
    {
        return processes;
    }
    let processes = on_admin(&state, (&profile_id, &database), |facet| {
        facet.list_processes()
    })
    .await?;
    processes
        .into_iter()
        .map(|process| ProcessDto::new(process, &application))
        .collect()
}

/// Refuse signals on read-only connections, like every other write.
fn ensure_signal_allowed(read_only: bool) -> Result<(), IpcError> {
    if read_only {
        return Err(IpcError::invalid(
            "This connection is read-only: cancelling queries and killing processes are disabled",
        ));
    }
    Ok(())
}

/// Cancel a process's statement or terminate it; the webview confirms first.
#[tracing::instrument(name = "process.signal", skip_all, fields(signal = ?request.signal))]
#[tauri::command]
#[specta::specta]
pub(crate) async fn process_signal(
    app: tauri::AppHandle,
    state: State<'_, Connections>,
    request: ProcessSignalRequest,
) -> Result<(), IpcError> {
    let profile = admin_profile(&state, &request.profile_id)?;
    ensure_signal_allowed(profile.safety.read_only())?;
    crate::security::confirm_batch(
        &app,
        &profile,
        vec!["Signal the reviewed server process".into()],
        &request.database,
    )
    .await?;
    let id = ProcessId::from(request.process_id);
    let (signal, allow_own) = (request.signal.into(), request.allow_own);
    let outcome = on_admin(
        &state,
        (&request.profile_id, &request.database),
        move |facet| {
            Box::pin(async move {
                if !allow_own && facet.own_process().await? == id {
                    return Ok(Signalled::Own);
                }
                facet.signal_process(id, signal).await?;
                Ok(Signalled::Done)
            })
        },
    )
    .await?;
    match outcome {
        Signalled::Done => {
            tracing::info!(process = id, "process signalled");
            Ok(())
        }
        Signalled::Own => Err(own_refused(id)),
    }
}

fn own_refused(id: ProcessId) -> IpcError {
    IpcError::invalid(format!(
        "Process {id} is the connection this list runs on; confirm ending it explicitly"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_connections_refuse_signals() {
        assert!(ensure_signal_allowed(false).is_ok());
        let refused = ensure_signal_allowed(true).err();
        assert_eq!(refused.map(|error| error.kind), Some(IpcErrorKind::Invalid));
    }

    #[test]
    fn signalling_the_list_connection_names_it() {
        let error = own_refused(42);
        assert_eq!(error.kind, IpcErrorKind::Invalid);
        assert!(error.message.contains("Process 42 is the connection"));
    }
}
