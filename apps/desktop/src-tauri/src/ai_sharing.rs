// SPDX-License-Identifier: Apache-2.0
//! Native identity resolution for local sharing settings and exact offline previews.
#![expect(
    clippy::needless_pass_by_value,
    reason = "Tauri owns command arguments"
)]
use crate::{
    ai::{self, AiState},
    connections::{Connections, IpcError, IpcErrorKind, profiles::profile_id},
    settings::{self, SettingsFile},
};
use openrows_ai::sharing::{
    AuditEntry, ContextBundle, ContextPreview, GateRequest, Refusal, SharingPolicy, SharingStore,
};
use openrows_sql::Dialect;
use openrows_store::profile::{ConnectionProfile, Driver, EnvKind};
use tauri::{AppHandle, Manager};

impl From<Refusal> for IpcError {
    fn from(error: Refusal) -> Self {
        let kind = match error {
            Refusal::Store => IpcErrorKind::Storage,
            Refusal::Invalid => IpcErrorKind::Invalid,
            Refusal::AiOff
            | Refusal::LevelTooLow
            | Refusal::Production
            | Refusal::ProviderNotAllowed
            | Refusal::LocalOnly
            | Refusal::Telemetry => IpcErrorKind::Unsupported,
        };
        Self::new(kind, error.to_string())
    }
}
/// Install local storage beside the existing profile and query stores.
pub(crate) fn install(app: &mut tauri::App) -> Result<(), Refusal> {
    let data = &app.state::<crate::locations::Locations>().data;
    app.manage(SharingStore::open(&data.join("ai-sharing.sqlite"))?);
    Ok(())
}
fn profile(app: &AppHandle, id: &str) -> Result<ConnectionProfile, IpcError> {
    Ok(app.state::<Connections>().profiles.get(profile_id(id)?)?)
}
fn dialect(driver: Driver) -> Dialect {
    match driver {
        Driver::Duckdb => Dialect::Duckdb,
        Driver::Postgres => Dialect::Postgres,
        Driver::Mysql | Driver::Mariadb => Dialect::MySql,
        Driver::Sqlserver => Dialect::SqlServer,
        Driver::Clickhouse => Dialect::ClickHouse,
        Driver::Sqlite | Driver::Libsql | Driver::LibsqlFile => Dialect::Sqlite,
    }
}
/// Read the device-local connection policy, default None even for imported profiles.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn ai_sharing_get(app: AppHandle, id: String) -> Result<SharingPolicy, IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_sharing_get")?;
    profile(&app, &id)?;
    Ok(app.state::<SharingStore>().policy(&id)?)
}
/// Persist explicit production consent and validate the allowlist against saved providers.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn ai_sharing_save(
    app: AppHandle,
    id: String,
    policy: SharingPolicy,
) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_sharing_save")?;
    let profile = profile(&app, &id)?;
    let value = settings::current(&app.state::<SettingsFile>()).unwrap_or_default();
    let settings = ai::parse(&value)?;
    if policy
        .providers
        .iter()
        .any(|id| !settings.providers.iter().any(|provider| &provider.id == id))
    {
        return Err(Refusal::ProviderNotAllowed.into());
    }
    app.state::<SharingStore>()
        .save(&id, policy, profile.env_tag.kind == EnvKind::Prod)?;
    // Invalidate previously admitted provider work, including metadata probes.
    let state = app.state::<AiState>();
    state.revoke();
    state.configure(&value);
    Ok(())
}
/// Offline exact preview through the same gate used by provider requests; no audit append.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn ai_context_preview(
    app: AppHandle,
    id: String,
    provider: String,
    bundle: ContextBundle,
) -> Result<ContextPreview, IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_context_preview")?;
    let profile = profile(&app, &id)?;
    let provider = ai::config(&app, &provider)?;
    let value = settings::current(&app.state::<SettingsFile>()).unwrap_or_default();
    let state = app.state::<AiState>();
    state.configure(&value);
    let gated = app.state::<SharingStore>().gate(GateRequest {
        connection: &id,
        production: profile.env_tag.kind == EnvKind::Prod,
        provider: &provider,
        control: &state.control,
        dialect: dialect(profile.driver),
        bundle,
    })?;
    Ok(gated.preview())
}
/// Scrubbed, newest-first outbound metadata only (at most 1,000 entries).
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn ai_audit_list(app: AppHandle) -> Result<Vec<AuditEntry>, IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_audit_list")?;
    Ok(app.state::<SharingStore>().audit()?)
}
/// Explicit local audit purge; policies and provider keys remain separate.
#[tauri::command(async)]
#[specta::specta]
pub(crate) fn ai_audit_purge(app: AppHandle) -> Result<(), IpcError> {
    app.state::<crate::security::Security>()
        .admit("ai_audit_purge")?;
    Ok(app.state::<SharingStore>().purge()?)
}
/// Reset sharing when an edited connection changes its target or environment.
pub(crate) fn reset_on_edit(
    app: &AppHandle,
    before: &ConnectionProfile,
    after: &ConnectionProfile,
) -> Result<(), IpcError> {
    if before.env_tag.kind != after.env_tag.kind
        || before.params != after.params
        || before.file != after.file
    {
        app.state::<SharingStore>().save(
            &after.id.to_string(),
            SharingPolicy::default(),
            after.env_tag.kind == EnvKind::Prod,
        )?;
        app.state::<AiState>().revoke();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_engine_has_the_existing_sql_lexer_and_refusals_are_scrubbed() {
        for (engine, expected) in [
            (Driver::Duckdb, Dialect::Duckdb),
            (Driver::Postgres, Dialect::Postgres),
            (Driver::Mysql, Dialect::MySql),
            (Driver::Mariadb, Dialect::MySql),
            (Driver::Sqlserver, Dialect::SqlServer),
            (Driver::Clickhouse, Dialect::ClickHouse),
            (Driver::Sqlite, Dialect::Sqlite),
            (Driver::Libsql, Dialect::Sqlite),
            (Driver::LibsqlFile, Dialect::Sqlite),
        ] {
            assert_eq!(dialect(engine), expected);
        }
        assert_eq!(IpcError::from(Refusal::Store).kind, IpcErrorKind::Storage);
        assert_eq!(
            IpcError::from(Refusal::Production).kind,
            IpcErrorKind::Unsupported
        );
    }
}
