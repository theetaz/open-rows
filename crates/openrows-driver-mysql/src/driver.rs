// SPDX-License-Identifier: Apache-2.0
//! The `MySQL` and `MariaDB` [`Driver`]s.
use openrows_driver_api::objects::{ObjectChange, ObjectPlan};
use openrows_driver_api::schema::{SchemaChange, TableDetail};
use openrows_driver_api::structure::{AlterPlan, AlterTable};
use openrows_driver_api::{
    BoxFuture, ConnectRequest, ConnectStage, ConnectTrace, Connection, Driver, DriverError,
    GenerateError,
};
use openrows_sql::Dialect;

use crate::ddl;
use crate::handshake::establish;
use crate::info;
use crate::session::{KillQuery, MySqlSession};
use crate::transport::{Target, within};
use crate::wire::Wire;

/// Database every account may open; a profile without a database connects
/// here to list the others (as `PostgreSQL` uses `postgres`).
const HOME_DATABASE: &str = "information_schema";

/// Which engine a profile names; both speak the same protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Flavour {
    /// `MySQL` (and `MySQL`-wire servers such as `TiDB`).
    MySql,
    /// `MariaDB`.
    MariaDb,
}

/// The `MySQL`/`MariaDB` driver for one engine.
#[derive(Clone, Copy, Debug)]
pub struct MySqlDriver(pub Flavour);

impl Driver for MySqlDriver {
    fn product(&self) -> &'static str {
        match self.0 {
            Flavour::MySql => "MySQL",
            Flavour::MariaDb => "MariaDB",
        }
    }

    fn dialect(&self) -> Dialect {
        Dialect::MySql
    }

    fn home_database(&self) -> &'static str {
        HOME_DATABASE
    }

    fn capabilities(&self) -> openrows_driver_api::Capabilities {
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

    fn object_sql(
        &self,
        change: &ObjectChange,
        source: Option<&TableDetail>,
    ) -> Result<ObjectPlan, GenerateError> {
        ddl::objects::object_sql(change, source)
    }
}

/// Connect, log in and read the server identity.
///
/// # Errors
/// `Network`, `Tls`, `Auth`, `Server` or `Protocol` failures.
pub async fn connect(
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<MySqlSession, DriverError> {
    let target = Target::of(&request.endpoint, request.via.as_ref())?;
    let established = establish(request, &target, trace).await?;
    let mut wire = Wire {
        packets: established.packets,
        capabilities: established.capabilities,
        in_transaction: false,
    };
    let fetch = info::fetch(&mut wire, &established.version, established.tls);
    let reading = within(request.timeouts.auth, "reading the server version", fetch);
    let summary = trace.stage(ConnectStage::ServerInfo, reading).await?;
    let cancel = KillQuery {
        request: request.clone(),
        target,
        connection_id: established.connection_id,
    };
    let mut session = MySqlSession::new(wire, summary, cancel);
    let commands = run_startup_commands(&mut session, &request.startup_commands);
    trace.stage(ConnectStage::StartupCommands, commands).await?;
    Ok(session)
}

/// The profile's startup commands, in order; the first failure fails the connect.
async fn run_startup_commands(
    session: &mut MySqlSession,
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
