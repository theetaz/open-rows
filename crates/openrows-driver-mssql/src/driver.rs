// SPDX-License-Identifier: Apache-2.0
//! The SQL Server [`Driver`].
use openrows_driver_api::objects::{ObjectChange, ObjectPlan};
use openrows_driver_api::schema::{SchemaChange, TableDetail};
use openrows_driver_api::structure::{AlterPlan, AlterTable};
use openrows_driver_api::{
    BoxFuture, Capabilities, ConnectRequest, ConnectStage, ConnectTrace, Connection, Driver,
    DriverError, GenerateError,
};
use openrows_sql::Dialect;

use crate::ddl;
use crate::handshake::establish;
use crate::info;
use crate::session::SqlServerSession;
use crate::transport::{Target, within};

/// Database every login may open; a profile without a database connects
/// here to list the others.
const HOME_DATABASE: &str = "master";

/// The SQL Server and Azure SQL driver.
#[derive(Clone, Copy, Debug, Default)]
pub struct SqlServerDriver;

impl Driver for SqlServerDriver {
    fn product(&self) -> &'static str {
        info::PRODUCT
    }

    fn dialect(&self) -> Dialect {
        Dialect::SqlServer
    }

    fn home_database(&self) -> &'static str {
        HOME_DATABASE
    }

    fn capabilities(&self) -> Capabilities {
        crate::capabilities::CAPABILITIES
    }

    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        Box::pin(async move {
            let session = connect(request, trace).await?;
            Ok(Box::new(session) as Box<dyn Connection>)
        })
    }

    fn table_ddl(
        &self,
        target: (&str, &str),
        detail: &TableDetail,
    ) -> Result<String, GenerateError> {
        Ok(ddl::table_ddl(target, detail))
    }

    fn object_sql(
        &self,
        change: &ObjectChange,
        source: Option<&TableDetail>,
    ) -> Result<ObjectPlan, GenerateError> {
        ddl::object_sql(change, source)
    }

    fn supports_alter(&self) -> bool {
        true
    }

    fn alter_sql(
        &self,
        target: &AlterTable<'_>,
        changes: &[SchemaChange],
    ) -> Result<AlterPlan, GenerateError> {
        ddl::alter_sql(target, changes).map_err(|error| GenerateError(error.to_string()))
    }
}

/// Connect, log in, read the server identity and run the startup commands.
///
/// # Errors
/// `Network`, `Tls`, `Auth`, `Server` or `Protocol` failures.
pub async fn connect(
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<SqlServerSession, DriverError> {
    let target = Target::of(&request.endpoint, request.via.as_ref())?;
    let established = establish(request, &target, trace).await?;
    let summary = info::pending(&established.env, established.tls.clone());
    let mut session = SqlServerSession::new(established.wire, established.env, summary);
    let fetch = info::fetch(&mut session, established.tls);
    let reading = within(request.timeouts.auth, "reading the server version", fetch);
    session.summary = trace.stage(ConnectStage::ServerInfo, reading).await?;
    let commands = run_startup_commands(&mut session, &request.startup_commands);
    trace.stage(ConnectStage::StartupCommands, commands).await?;
    Ok(session)
}

/// The profile's startup commands, in order; the first failure fails the connect.
async fn run_startup_commands(
    session: &mut SqlServerSession,
    commands: &[String],
) -> Result<(), DriverError> {
    for (index, sql) in commands.iter().enumerate() {
        session
            .control(sql, &[])
            .await
            .map_err(|error| error.in_startup_command(index))?;
    }
    Ok(())
}
