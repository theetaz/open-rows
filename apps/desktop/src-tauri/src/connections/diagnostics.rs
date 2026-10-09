// SPDX-License-Identifier: Apache-2.0
//! Test connection with step-by-step diagnostics (M1-07): proxy and SSH
//! hops, tunnel (to a port or a Unix socket) or direct connect, TLS
//! handshake with the server certificate's details and expiry, and
//! authentication, each timed, the failing step carrying a typed error;
//! on success the latency of a trivial query. The test uses the form's
//! values and its own tunnel, closed afterwards.
mod hints;
mod probe;
mod run;
mod steps;

use std::sync::Arc;
use std::time::Instant;

use openrows_net::ProxySpec;
use openrows_store::profile::{ConnectionProfile, ProfileService, SecretChanges, SecretSlot};
use serde::Serialize;
use specta::Type;
use tauri::State;

use probe::{CertificateDto, LatencyDto};
use steps::{Steps, TestStepDto};

use super::Connections;
use super::dto::ProfileInput;
use super::error::IpcError;
use super::profiles::{profile_id, resolve_password, resolve_secret};
use super::proxy_dto::proxy_spec;
use super::target::{self, DriverSecrets, ServerDto};
use super::tunnels::SshSecrets;

/// The outcome of Test connection: every step that ran, the server on
/// success, the typed error of the failing step otherwise.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TestReport {
    steps: Vec<TestStepDto>,
    server: Option<ServerDto>,
    error: Option<IpcError>,
    /// The server certificate, when TLS was negotiated.
    certificate: Option<CertificateDto>,
    /// Round trips of `SELECT 1` after connecting.
    latency: Option<LatencyDto>,
    elapsed_ms: u32,
}

/// The form's profile with the secrets it asks for (typed or stored).
pub(crate) struct TestPlan {
    pub(crate) profile: ConnectionProfile,
    pub(crate) driver: DriverSecrets,
    pub(crate) ssh: SshSecrets,
    pub(crate) proxy: Option<Arc<ProxySpec>>,
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
    let mut profile = input.new_profile();
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
    let proxy_password = secret(SecretSlot::ProxyPassword)?;
    Ok(TestPlan {
        driver: DriverSecrets {
            password: resolve_password(service, stored.as_ref(), &input.password)?,
            tls_key_passphrase: secret(SecretSlot::TlsKeyPassphrase)?,
        },
        ssh: SshSecrets {
            password: secret(SecretSlot::SshPassword)?,
            passphrase: secret(SecretSlot::SshPassphrase)?,
        },
        proxy: profile
            .proxy
            .as_ref()
            .map(|settings| proxy_spec(settings, proxy_password.as_ref())),
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
    let target = target::target(&plan.profile, &plan.driver)?;
    let started = Instant::now();
    let mut steps = Steps {
        engine: plan.profile.driver,
        proxy: plan.proxy.as_ref().map(|proxy| proxy.describe()),
        ..Steps::default()
    };
    let outcome = run::run(&state, plan, target, &mut steps).await;
    let (server, error) = match outcome.result {
        Ok(server) => (Some(server), None),
        Err(error) => (None, Some(error)),
    };
    Ok(TestReport {
        steps: steps.list,
        server,
        error,
        certificate: outcome.certificate,
        latency: outcome.latency,
        elapsed_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
    })
}

#[cfg(test)]
mod tests;
