// SPDX-License-Identifier: Apache-2.0
//! The `MySQL` and `MariaDB` [`Driver`]s.
use openrows_driver_api::{
    BoxFuture, ConnectRequest, ConnectStage, ConnectTrace, Connection, Driver, DriverError,
};
use openrows_sql::Dialect;

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
    };
    let fetch = info::fetch(&mut wire, &established.version, established.tls);
    let reading = within(request.timeouts.auth, "reading the server version", fetch);
    let summary = trace.stage(ConnectStage::ServerInfo, reading).await?;
    let cancel = KillQuery {
        request: request.clone(),
        target,
        connection_id: established.connection_id,
    };
    Ok(MySqlSession::new(wire, summary, cancel))
}
