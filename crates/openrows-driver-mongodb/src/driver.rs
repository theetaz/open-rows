// SPDX-License-Identifier: Apache-2.0
use crate::{MongoSession, error, proxy::Proxy};
use mongodb::{
    Client,
    options::{ClientOptions, Credential, ServerAddress, Socks5Proxy, Tls},
};
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{
    BoxFuture, Capabilities, ConnectRequest, ConnectStage, ConnectTrace, Connection, DiscardSink,
    Driver, DriverError, Endpoint, Feature, Location, ServerSummary, Session,
};
use openrows_sql::Dialect;

/// Official `MongoDB` driver behind the shared networking layer.
#[derive(Clone, Copy, Debug, Default)]
pub struct MongoDriver;
impl Driver for MongoDriver {
    fn product(&self) -> &'static str {
        "MongoDB"
    }
    fn dialect(&self) -> Dialect {
        Dialect::MongoDb
    }
    fn home_database(&self) -> &'static str {
        "test"
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            location: Location::Server,
            features: &[
                Feature::Catalog,
                Feature::MultipleDatabases,
                Feature::RowEdits,
                Feature::Tls,
                Feature::Routes,
                Feature::Cancel,
                Feature::StartupCommands,
            ],
            object_kinds: &[ObjectKind::Table, ObjectKind::View],
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

/// Connect, authenticate and load server identity, with staged diagnostics.
/// # Errors
/// Endpoint, DNS, route, TLS, authentication or startup-command failures.
pub async fn connect(
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<MongoSession, DriverError> {
    let options = trace
        .stage(ConnectStage::Resolve, async {
            tokio::time::timeout(request.timeouts.dns, options(request))
                .await
                .map_err(|_| DriverError::Network(std::io::ErrorKind::TimedOut.into()))?
        })
        .await?;
    let destination = address(options.hosts.first().ok_or_else(error::invalid)?)?;
    let tls = transport(request, (&destination.0, destination.1), trace).await?;
    let proxy = Proxy::start(request).await?;
    let (client, control) = client(options, request, &proxy, (&destination.0, destination.1))?;
    let summary = match server(&client, request, tls, trace).await {
        Ok(summary) => summary,
        Err(error) => {
            client.shutdown().await;
            return Err(error);
        }
    };
    let mut session = MongoSession::new(client, summary, proxy, control);
    if let Err(error) = trace
        .stage(
            ConnectStage::StartupCommands,
            startup(&mut session, &request.startup_commands),
        )
        .await
    {
        let _ = Box::new(session).close().await;
        return Err(error);
    }
    Ok(session)
}
async fn transport(
    request: &ConnectRequest,
    (host, port): (&str, u16),
    trace: &mut ConnectTrace,
) -> Result<openrows_driver_api::TlsStatus, DriverError> {
    let stream = trace
        .stage(
            ConnectStage::Connect,
            crate::proxy::dial(request, host, port),
        )
        .await?;
    let (_, tls) = trace
        .stage(
            ConnectStage::Tls,
            crate::proxy::secure(request, (host, port), stream),
        )
        .await?;
    trace.tls = Some(tls.clone());
    Ok(tls)
}
fn client(
    mut options: ClientOptions,
    request: &ConnectRequest,
    proxy: &Proxy,
    (host, port): (&str, u16),
) -> Result<(Client, std::sync::Arc<crate::cancel::State>), DriverError> {
    let control = std::sync::Arc::new(crate::cancel::State::default());
    options.command_event_handler = Some(crate::cancel::State::handler(control.clone()));
    options.socks5_proxy = Some(
        Socks5Proxy::builder()
            .host("127.0.0.1".to_owned())
            .port(proxy.port)
            .authentication(("openrows".into(), proxy.password.clone()))
            .build(),
    );
    options.tls = Some(Tls::Disabled);
    options.retry_reads = Some(false);
    options.retry_writes = Some(false);
    options.max_pool_size = Some(4);
    options.app_name = Some(request.application_name.clone());
    options.server_selection_timeout = Some(request.timeouts.auth);
    options.connect_timeout = Some(request.timeouts.tcp);
    if request
        .via
        .as_ref()
        .is_some_and(|via| via.retarget(host, port).is_none())
        || request
            .driver_options
            .get("mongodb_direct")
            .is_some_and(|v| v == "true")
    {
        options.direct_connection = Some(true);
    }
    Client::with_options(options)
        .map(|client| (client, control))
        .map_err(error::mongo)
}
async fn server(
    client: &Client,
    request: &ConnectRequest,
    tls: openrows_driver_api::TlsStatus,
    trace: &mut ConnectTrace,
) -> Result<ServerSummary, DriverError> {
    trace
        .stage(ConnectStage::Authenticate, async {
            tokio::time::timeout(request.timeouts.auth, ping(client))
                .await
                .map_err(|_| DriverError::Network(std::io::ErrorKind::TimedOut.into()))?
        })
        .await?;
    let info = trace
        .stage(ConnectStage::ServerInfo, async {
            client
                .database("admin")
                .run_command(mongodb::bson::doc! {"buildInfo": 1})
                .await
                .map_err(error::mongo)
        })
        .await?;
    Ok(ServerSummary {
        product: "MongoDB".into(),
        version: info.get_str("version").unwrap_or("unknown").into(),
        flavour: None,
        database: if request.database.is_empty() {
            "test".into()
        } else {
            request.database.clone()
        },
        user: request.user.clone(),
        tls,
    })
}
async fn startup(session: &mut MongoSession, commands: &[String]) -> Result<(), DriverError> {
    for (index, command) in commands.iter().enumerate() {
        session
            .execute(command, &mut DiscardSink)
            .await
            .map_err(|error| error.in_startup_command(index))?;
    }
    Ok(())
}

async fn options(request: &ConnectRequest) -> Result<ClientOptions, DriverError> {
    let Endpoint::Server { host, port } = &request.endpoint else {
        return Err(DriverError::Unsupported(
            "MongoDB requires a server endpoint".into(),
        ));
    };
    if host.contains(['/', '?', '#', '@', '\0', '\\']) {
        return Err(DriverError::Unsupported(
            "MongoDB host must be a DNS name or IP address".into(),
        ));
    }
    let srv = request
        .driver_options
        .get("mongodb_srv")
        .is_some_and(|v| v == "true");
    let uri = if srv {
        format!("mongodb+srv://{host}/")
    } else {
        format!(
            "mongodb://{}:{port}/",
            if host.contains(':') {
                format!("[{host}]")
            } else {
                host.clone()
            }
        )
    };
    let mut options = ClientOptions::parse(&uri).await.map_err(error::mongo)?;
    options.credential = credential(request, srv, options.credential.as_ref())?;
    Ok(options)
}
fn credential(
    request: &ConnectRequest,
    srv: bool,
    discovered: Option<&Credential>,
) -> Result<Option<Credential>, DriverError> {
    if request.user.is_empty() {
        return if request.password.is_some() {
            Err(DriverError::Auth(
                "Enter a MongoDB user or clear the password".into(),
            ))
        } else {
            Ok(None)
        };
    }
    let fallback = if srv || request.database.is_empty() {
        "admin".into()
    } else {
        request.database.clone()
    };
    let source = request
        .driver_options
        .get("authSource")
        .cloned()
        .or_else(|| discovered.and_then(|c| c.source.clone()))
        .unwrap_or(fallback);
    Ok(Some(
        Credential::builder()
            .username(request.user.clone())
            .password(request.password.as_ref().map(|p| p.expose().to_owned()))
            .source(source)
            .build(),
    ))
}
fn address(address: &ServerAddress) -> Result<(String, u16), DriverError> {
    match address {
        ServerAddress::Tcp { host, port } => Ok((host.clone(), port.unwrap_or(27017))),
        _ => Err(DriverError::Unsupported("MongoDB Unix sockets".into())),
    }
}
pub(crate) async fn ping(client: &Client) -> Result<(), DriverError> {
    client
        .database("admin")
        .run_command(mongodb::bson::doc! {"ping": 1})
        .await
        .map_err(error::mongo)
        .map(drop)
}
