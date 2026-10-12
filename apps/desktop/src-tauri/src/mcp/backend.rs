// SPDX-License-Identifier: Apache-2.0
//! Resolve app-owned profiles and currently open databases before any MCP work.
use crate::{
    ai,
    connections::{Connections, catalog::read_catalog, profiles::profile_id, route, target},
    settings::{self, SettingsFile},
};
use openrows_ai::sharing::ContextBundle;
use openrows_core::{
    SessionState,
    safety::{Gate, SafetyPlan, SafetyPolicy},
};
use openrows_driver_api::BoxFuture;
use openrows_mcp::{McpError, ReadEngine, ToolBackend, ToolContext, ToolInput, protected_read};
use openrows_store::profile::{ConnectionProfile, Driver, EnvKind};
use tauri::{AppHandle, Manager};

pub(crate) struct Backend(pub(crate) AppHandle);
impl Backend {
    fn profile(&self, input: &ToolInput) -> Result<ConnectionProfile, McpError> {
        self.0
            .state::<crate::security::Security>()
            .admit("mcp_read")
            .map_err(|_| McpError::Cancelled)?;
        if !self.0.state::<ai::AiState>().control.enabled() {
            return Err(McpError::Cancelled);
        }
        let id = input.connection.as_deref().ok_or(McpError::Invalid)?;
        let connections = self.0.state::<Connections>();
        let profile = connections
            .profiles
            .get(profile_id(id).map_err(|_| McpError::Invalid)?)
            .map_err(|_| McpError::Invalid)?;
        let open = connections.owners.all().iter().any(|(session, owner)| {
            owner.profile == profile.id
                && input
                    .database
                    .as_ref()
                    .is_none_or(|database| &owner.database == database)
                && matches!(
                    connections.sessions.state(*session),
                    Some(SessionState::Idle | SessionState::Busy)
                )
        });
        if !open {
            return Err(McpError::Cancelled);
        }
        Ok(profile)
    }
    async fn rows(&self, tool: &str, input: &ToolInput) -> Result<ContextBundle, McpError> {
        let profile = self.profile(input)?;
        let sql = input.sql.as_deref().ok_or(McpError::Invalid)?;
        let policy = crate::queries::commands::policy(&profile);
        require_read(policy, sql)?;
        openrows_sql::validate_ai_read(sql, policy.dialect).map_err(|_| McpError::Unsafe)?;
        let engine = engine(profile.driver);
        if matches!(
            engine,
            ReadEngine::Mongodb
                | ReadEngine::Libsql
                | ReadEngine::Sqlserver
                | ReadEngine::Clickhouse
                | ReadEngine::Duckdb
        ) {
            return Err(McpError::Unsafe);
        }
        let connections = self.0.state::<Connections>();
        let database = input.database.as_deref().ok_or(McpError::Invalid)?;
        let mut target = route::profile_target(&connections, &profile.id.to_string())
            .await
            .map_err(|_| McpError::Database)?
            .with_database(database)
            .with_application_suffix("mcp");
        // App startup SQL is user-owned configuration, never part of a model read.
        target.request.startup_commands.clear();
        let connected = target::connect(&target)
            .await
            .map_err(|_| McpError::Database)?;
        self.profile(input)?;
        protected_read(connected.connection, engine, sql, tool == "explain_query").await
    }
}
impl ToolBackend for Backend {
    fn context(&self, provider: &str, input: &ToolInput) -> Result<ToolContext, McpError> {
        let profile = self.profile(input)?;
        require_read(crate::queries::commands::policy(&profile), "SELECT 1")?;
        let value = settings::current(&self.0.state::<SettingsFile>()).unwrap_or_default();
        let settings = ai::parse(&value).map_err(|_| McpError::Cancelled)?;
        let provider = settings
            .providers
            .into_iter()
            .find(|item| item.id == provider)
            .ok_or(McpError::Sharing)?;
        Ok(ToolContext {
            production: profile.env_tag.kind == EnvKind::Prod,
            dialect: crate::connections::drivers::driver(profile.driver).dialect(),
            provider,
        })
    }
    fn read<'a>(
        &'a self,
        tool: &'a str,
        input: &'a ToolInput,
    ) -> BoxFuture<'a, Result<ContextBundle, McpError>> {
        Box::pin(async move {
            self.profile(input)?;
            match tool {
                "run_read_query" | "explain_query" => self.rows(tool, input).await,
                "list_schemas" | "list_objects" | "describe_object" => {
                    self.catalog(tool, input).await
                }
                _ => Err(McpError::Invalid),
            }
        })
    }
    fn propose(&self, input: &ToolInput) -> Result<(), McpError> {
        self.profile(input)?;
        super::proposals::enqueue(&self.0, input)
    }
}
impl Backend {
    async fn catalog(&self, tool: &str, input: &ToolInput) -> Result<ContextBundle, McpError> {
        let id = input.connection.as_deref().ok_or(McpError::Invalid)?;
        let database = input.database.as_deref().ok_or(McpError::Invalid)?;
        let schema = input.schema.clone().unwrap_or_default();
        let object = input.object.clone().unwrap_or_default();
        let tool = tool.to_owned();
        let objects = read_catalog(
            &self.0.state::<Connections>(),
            (id, database),
            move |catalog| Box::pin(super::catalog::collect(catalog, tool, (schema, object))),
        )
        .await
        .map_err(|_| McpError::Database)?;
        self.profile(input)?;
        Ok(ContextBundle {
            schema: objects,
            ..ContextBundle::default()
        })
    }
}
fn engine(driver: Driver) -> ReadEngine {
    match driver {
        Driver::Postgres => ReadEngine::Postgres,
        Driver::Mysql => ReadEngine::Mysql,
        Driver::Mariadb => ReadEngine::Mariadb,
        Driver::Sqlite => ReadEngine::Sqlite,
        Driver::LibsqlFile => ReadEngine::LibsqlFile,
        Driver::Mongodb => ReadEngine::Mongodb,
        Driver::Libsql => ReadEngine::Libsql,
        Driver::Sqlserver => ReadEngine::Sqlserver,
        Driver::Clickhouse => ReadEngine::Clickhouse,
        Driver::Duckdb => ReadEngine::Duckdb,
    }
}

fn require_read(policy: SafetyPolicy, sql: &str) -> Result<(), McpError> {
    let plan = SafetyPlan::check(policy, vec![sql.into()]);
    if plan.gate() != Gate::Run || plan.authentication_required {
        return Err(McpError::Review);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use openrows_types::safety::Safety;
    #[test]
    fn all_tools_respect_confirmation_authentication_and_read_only_policy() {
        for mode in [Safety::ConfirmAll, Safety::AuthenticateAll] {
            assert!(
                require_read(
                    SafetyPolicy {
                        mode,
                        ..SafetyPolicy::default()
                    },
                    "SELECT 1"
                )
                .is_err()
            );
        }
        let read_only = SafetyPolicy {
            mode: Safety::ReadOnly,
            ..SafetyPolicy::default()
        };
        assert!(require_read(read_only, "SELECT 1").is_ok());
        assert!(require_read(read_only, "DELETE FROM t").is_err());
    }
}
