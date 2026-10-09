// SPDX-License-Identifier: Apache-2.0
//! Driver registration, authentication, identity and capability claims.
use crate::{
    http::{Client, read_limited},
    session::ClickHouseSession,
    transport::within,
};
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{
    BoxFuture, Capabilities, ConnectRequest, ConnectStage, ConnectTrace, Connection, Driver,
    DriverError, Feature, Location, ServerSummary,
};
use openrows_sql::Dialect;

/// `ClickHouse` Tier 2 HTTP driver, with read-only result grids.
#[derive(Clone, Copy, Debug, Default)]
pub struct ClickHouseDriver;
impl Driver for ClickHouseDriver {
    fn product(&self) -> &'static str {
        "ClickHouse"
    }
    fn dialect(&self) -> Dialect {
        Dialect::ClickHouse
    }
    fn home_database(&self) -> &'static str {
        "default"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            location: Location::Server,
            features: &[
                Feature::Cancel,
                Feature::BindParameters,
                Feature::MultipleDatabases,
                Feature::Catalog,
                Feature::Tls,
                Feature::Routes,
                Feature::ProcessList,
                Feature::StartupCommands,
            ],
            object_kinds: &[
                ObjectKind::Table,
                ObjectKind::View,
                ObjectKind::MaterializedView,
                ObjectKind::Function,
            ],
        }
    }
    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        Box::pin(async move { Ok(Box::new(connect(request, trace).await?) as Box<dyn Connection>) })
    }
}

/// Authenticate, read the server identity, then apply startup commands.
///
/// # Errors
/// Network, TLS, authentication, protocol or startup-command failures.
pub async fn connect(
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<ClickHouseSession, DriverError> {
    let mut request = request.clone();
    if request.database.is_empty() {
        "default".clone_into(&mut request.database);
    }
    if request.user.is_empty() {
        "default".clone_into(&mut request.user);
    }
    let client = Client::new(request.clone())?;
    let id = uuid::Uuid::now_v7().to_string();
    let auth = client.send_traced(
        "SELECT version(), currentDatabase(), currentUser() FORMAT JSONCompactEachRow",
        &[],
        (&id, Some(&id)),
        trace,
    );
    let result = within(request.timeouts.auth, "ClickHouse authentication", auth).await;
    let (mut reply, tls) = match result {
        Ok(result) => result,
        Err(error) => {
            trace.failed.get_or_insert(ConnectStage::Authenticate);
            return Err(error);
        }
    };
    trace.tls = Some(tls.clone());
    trace
        .stage(ConnectStage::Authenticate, async { Ok(()) })
        .await?;
    let bytes = trace
        .stage(
            ConnectStage::ServerInfo,
            read_limited(reply.response.body_mut(), 65_536),
        )
        .await?;
    let identity: Vec<String> = serde_json::from_slice(&bytes)
        .map_err(|_| DriverError::Protocol("invalid ClickHouse identity".to_owned()))?;
    let summary = ServerSummary {
        product: "ClickHouse".to_owned(),
        version: identity.first().cloned().unwrap_or_default(),
        database: identity.get(1).cloned().unwrap_or(request.database),
        user: identity.get(2).cloned().unwrap_or(request.user),
        flavour: None,
        tls,
    };
    let mut session = ClickHouseSession::new(client, id, summary);
    let startup = async {
        for (index, sql) in request.startup_commands.iter().enumerate() {
            use openrows_driver_api::Session;
            session
                .execute(sql, &mut openrows_driver_api::DiscardSink)
                .await
                .map_err(|e| e.in_startup_command(index))?;
        }
        Ok(())
    };
    trace.stage(ConnectStage::StartupCommands, startup).await?;
    Ok(session)
}
