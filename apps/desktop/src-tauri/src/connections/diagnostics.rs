// SPDX-License-Identifier: Apache-2.0
//! Test connection with step-by-step diagnostics (M1-07 for D-10/D-11):
//! SSH hops, tunnel or direct connect, TLS handshake (protocol, cipher,
//! certificate subject) and authentication, each timed, the failing step
//! carrying a typed error. The test uses the form's values and its own
//! tunnel, closed afterwards.
mod hints;
mod steps;

use std::sync::Arc;
use std::time::Instant;

use openrows_driver_api::Session;
use openrows_driver_postgres::PgSession;
use openrows_net::{SshTunnel, TunnelDialer};
use openrows_store::profile::{
    ConnectionProfile, EnvKind, EnvTag, ProfileService, SecretChanges, SecretSlot,
};
use serde::Serialize;
use specta::Type;
use tauri::State;

use steps::TestStepDto;
use steps::{DatabaseRun, Steps};

use super::Connections;
use super::dto::ProfileInput;
use super::error::IpcError;
use super::pg::{self, DriverSecrets, ServerDto, Target};
use super::profiles::{profile_id, resolve_password, resolve_secret};
use super::tunnels::{SshSecrets, route_spec};

/// The outcome of Test connection: every step that ran, the server on
/// success, the typed error of the failing step otherwise.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestReport {
    steps: Vec<TestStepDto>,
    server: Option<ServerDto>,
    error: Option<IpcError>,
    elapsed_ms: u32,
}

/// The form's profile with the secrets it asks for (typed or stored).
pub(crate) struct TestPlan {
    pub(crate) profile: ConnectionProfile,
    pub(crate) driver: DriverSecrets,
    pub(crate) ssh: SshSecrets,
}

/// Build the draft profile and resolve its secrets; the keychain is read
/// only for secrets that the edited profile stores and the form keeps.
///
/// # Errors
/// `Invalid` fields, unknown profiles, a bad pasted URL or keychain failures.
pub(crate) fn plan(
    service: &ProfileService,
    id: Option<&str>,
    input: &ProfileInput,
) -> Result<TestPlan, IpcError> {
    let stored = id
        .map(|id| profile_id(id).and_then(|id| Ok(service.get(id)?)))
        .transpose()?;
    let mut profile =
        ConnectionProfile::new_postgres(&input.name, input.params(), EnvTag::new(EnvKind::Local));
    input.apply(&mut profile)?;
    profile.validate()?;
    let changes: SecretChanges = input.secret_changes()?;
    let secret = |slot: SecretSlot| {
        let wanted = profile.uses_secret(slot);
        let change = changes.get(slot).clone();
        wanted
            .then(|| resolve_secret(service, stored.as_ref(), slot, change))
            .transpose()
            .map(Option::flatten)
    };
    Ok(TestPlan {
        driver: DriverSecrets {
            password: resolve_password(service, stored.as_ref(), &input.password)?,
            tls_key_passphrase: secret(SecretSlot::TlsKeyPassphrase)?,
        },
        ssh: SshSecrets {
            password: secret(SecretSlot::SshPassword)?,
            passphrase: secret(SecretSlot::SshPassphrase)?,
        },
        profile,
    })
}

/// Connect with the form values step by step, report, and disconnect.
#[tauri::command]
#[specta::specta]
pub(crate) async fn connection_test(
    state: State<'_, Connections>,
    profile_id: Option<String>,
    input: ProfileInput,
) -> Result<TestReport, IpcError> {
    let plan = plan(&state.profiles, profile_id.as_deref(), &input)?;
    let target = pg::target(&plan.profile.params, &plan.profile.tls, &plan.driver)?;
    let started = Instant::now();
    let mut steps = Steps::default();
    let outcome = run(&state, plan, target, &mut steps).await;
    let (server, error) = match outcome {
        Ok(server) => (Some(server), None),
        Err(error) => (None, Some(error)),
    };
    Ok(TestReport {
        steps: steps.0,
        server,
        error,
        elapsed_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
    })
}

async fn run(
    state: &Connections,
    plan: TestPlan,
    mut target: Target,
    steps: &mut Steps,
) -> Result<ServerDto, IpcError> {
    let profile = plan.profile;
    let tunnel = match &profile.ssh {
        Some(ssh) => {
            let spec = route_spec(ssh, plan.ssh)?;
            let attempt = state.opener.open(state.opener.plan(&spec).await?).await;
            if let Some(error) = steps.ssh(&attempt) {
                return Err(error);
            }
            let tunnel = Arc::new(
                attempt
                    .result
                    .map_err(|error| super::tunnels::ssh_error(&error))?,
            );
            let dialer = TunnelDialer::new(
                Arc::clone(&tunnel),
                &profile.params.host,
                profile.params.port,
            );
            target = target.through(Arc::new(dialer));
            Some(tunnel)
        }
        None => None,
    };
    let result = database(&target, tunnel.is_some(), steps).await;
    // The target's dialer shares the tunnel; release it so the tunnel can close.
    drop(target);
    if let Some(tunnel) = tunnel.and_then(|tunnel| Arc::try_unwrap(tunnel).ok()) {
        // The test is over; a failed goodbye on a throwaway tunnel changes nothing.
        let _ = SshTunnel::close(tunnel).await;
    }
    result
}

async fn database(
    target: &Target,
    tunnelled: bool,
    steps: &mut Steps,
) -> Result<ServerDto, IpcError> {
    let (result, trace) = PgSession::connect_traced(&target.config, &target.options).await;
    let server = result
        .as_ref()
        .ok()
        .map(|session| ServerDto::from(session.server_info()));
    let run = DatabaseRun {
        trace: &trace,
        host: &target.config.host,
        port: target.config.port,
        user: &target.config.user,
        tunnelled,
        server: server.as_ref(),
    };
    steps.database(&run, result.as_ref().err());
    match (result, server) {
        (Ok(session), Some(server)) => {
            // The test already succeeded; a failed goodbye changes nothing.
            let _ = Box::new(session).close().await;
            Ok(server)
        }
        (Err(error), _) => Err(IpcError::from(&error)),
        (Ok(_), None) => Err(IpcError::new(
            super::error::IpcErrorKind::Internal,
            "The connection reported no server",
        )),
    }
}

#[cfg(test)]
mod tests;
