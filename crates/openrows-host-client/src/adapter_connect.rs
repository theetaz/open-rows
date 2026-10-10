// SPDX-License-Identifier: Apache-2.0
//! Connect admission precedes host launch and retains the canonical file lease.
use crate::{FileReservation, HostedConnection};
use openrows_driver_api::{ConnectRequest, ConnectTrace, DriverError, Endpoint};
use openrows_host_protocol::{OwnedConnect, WireEndpoint, WireFile};

pub(crate) fn prepare(
    request: &ConnectRequest,
    dialect: openrows_sql::Dialect,
) -> Result<(OwnedConnect, Option<FileReservation>, bool), DriverError> {
    let duckdb = dialect == openrows_sql::Dialect::Duckdb;
    if !duckdb && dialect != openrows_sql::Dialect::Sqlite {
        return Err(DriverError::Unsupported(
            "host registry supports DuckDB and test SQLite only".into(),
        ));
    }
    let mut config = OwnedConnect::try_from(request)?;
    let reservation = if duckdb {
        let Endpoint::File { source, attached } = &request.endpoint else {
            return Err(DriverError::Unsupported(
                "DuckDB opens a local file or memory database".into(),
            ));
        };
        if !attached.is_empty() {
            return Err(DriverError::Unsupported(
                "DuckDB attachments are not supported yet".into(),
            ));
        }
        for sql in &request.startup_commands {
            crate::duckdb::admit(sql)?;
        }
        Some(crate::file_reservations::process_registry().reserve(source)?)
    } else {
        None
    };
    if let Some(path) = reservation
        .as_ref()
        .and_then(FileReservation::canonical_path)
        && let WireEndpoint::File {
            source: WireFile::Disk { path: supplied, .. },
            ..
        } = &mut config.endpoint
    {
        *supplied = path
            .to_str()
            .ok_or_else(|| DriverError::Unsupported("DuckDB file path must be UTF-8".into()))?
            .into();
    }
    Ok((config, reservation, duckdb))
}
impl HostedConnection {
    pub(crate) fn connected(
        &mut self,
        connected: openrows_host_protocol::Connected,
        trace: &mut ConnectTrace,
    ) -> Result<(), DriverError> {
        if connected.facets
            != (openrows_host_protocol::Facets {
                catalog: !self.duckdb,
                editor: !self.duckdb,
                admin: false,
                bulk: false,
            })
            || connected.tls != 2
        {
            return Err(self.protocol());
        }
        *trace = crate::adapter_trace::decode(connected.trace)?;
        self.server = openrows_driver_api::ServerSummary {
            product: connected.product,
            version: connected.version,
            flavour: connected.flavour,
            database: connected.database,
            user: connected.user,
            tls: openrows_driver_api::TlsStatus::LocalSocket,
        };
        self.transaction = connected.in_transaction;
        self.facets = connected.facets;
        if let Some(reservation) = &self.reservation {
            reservation.refresh();
        }
        Ok(())
    }
}
