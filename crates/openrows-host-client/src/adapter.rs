// SPDX-License-Identifier: Apache-2.0
//! Tauri-free driver proxy; synchronous SQL generation stays with parent metadata.
use crate::{HostSession, HostSupervisor, adapter_cancel::Cancellation};
use openrows_driver_api::{
    BoxFuture, Capabilities, ConnectRequest, ConnectTrace, Connection, Driver, DriverError,
    Feature, GenerateError, Location, ServerSummary, TlsStatus,
};
use openrows_driver_api::{
    catalog::ObjectKind,
    objects::{ObjectChange, ObjectPlan},
    schema::{SchemaChange, TableDetail},
    structure::{AlterPlan, AlterTable},
};
use openrows_host_protocol::{Control, Facets, Reply};
use std::sync::Arc;

/// Hosted `SQLite`'s test contract; bulk inputs await a separately reviewed chunk protocol.
pub const HOSTED_SQLITE_CAPABILITIES: Capabilities = Capabilities {
    location: Location::File,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::Schemas,
        Feature::Catalog,
        Feature::RowEdits,
        Feature::StructureChanges,
        Feature::TransactionalDdl,
        Feature::StartupCommands,
        Feature::SchemaFingerprint,
        Feature::TableLifecycle,
        Feature::ViewEditing,
        Feature::TriggerEditing,
        Feature::ForeignKeyEditing,
        Feature::CheckEditing,
        Feature::ColumnCollation,
        Feature::PartialIndexes,
    ],
    object_kinds: &[ObjectKind::Table, ObjectKind::View],
};
/// Lazy host launcher plus pure parent-side dialect and SQL generators.
pub struct HostedDriver {
    supervisor: HostSupervisor,
    metadata: Arc<dyn Driver>,
}
impl HostedDriver {
    /// Construct without launching; `SQLite` callers use this only in test fixtures.
    #[must_use]
    pub fn new(supervisor: HostSupervisor, metadata: Arc<dyn Driver>) -> Self {
        Self {
            supervisor,
            metadata,
        }
    }
}
impl HostedDriver {
    /// Open a typed proxy for headless owners that need host diagnostics.
    /// # Errors
    /// Driver connect, unsupported route or typed host failure; never retried.
    pub async fn open(
        &self,
        request: &ConnectRequest,
        trace: &mut ConnectTrace,
    ) -> Result<HostedConnection, DriverError> {
        let (config, mut reservation, duckdb) =
            crate::adapter_connect::prepare(request, self.metadata.dialect())?;
        let host = self.supervisor.spawn().await.map_err(DriverError::Host)?;
        if let Some(reservation) = &mut reservation {
            reservation.bind(&host);
        }
        let mut connection = HostedConnection::new(host);
        connection.duckdb = duckdb;
        connection.reservation = reservation;
        let reply = connection.call(&Control::Connect(config)).await;
        if let Some(failed) = connection.failed_connect.take() {
            *trace = crate::adapter_trace::decode(failed).map_err(|_| connection.protocol())?;
        }
        let reply = match reply {
            Ok(reply) => reply,
            Err(error) => {
                if !matches!(error, DriverError::Host(_)) {
                    let _ = connection.host.close().await;
                }
                return Err(error);
            }
        };
        let Reply::Connected(connected) = reply else {
            return Err(connection.protocol());
        };
        connection.connected(connected, trace)?;
        Ok(connection)
    }
}
impl Driver for HostedDriver {
    fn product(&self) -> &'static str {
        self.metadata.product()
    }
    fn dialect(&self) -> openrows_sql::Dialect {
        self.metadata.dialect()
    }
    fn home_database(&self) -> &'static str {
        self.metadata.home_database()
    }
    fn capabilities(&self) -> Capabilities {
        if self.metadata.dialect() == openrows_sql::Dialect::Duckdb {
            crate::DUCKDB_CAPABILITIES
        } else {
            HOSTED_SQLITE_CAPABILITIES
        }
    }
    fn supports_alter(&self) -> bool {
        self.metadata.supports_alter()
    }
    fn table_ddl(
        &self,
        target: (&str, &str),
        detail: &TableDetail,
    ) -> Result<String, GenerateError> {
        self.metadata.table_ddl(target, detail)
    }
    fn alter_sql(
        &self,
        target: &AlterTable<'_>,
        changes: &[SchemaChange],
    ) -> Result<AlterPlan, GenerateError> {
        self.metadata.alter_sql(target, changes)
    }
    fn object_sql(
        &self,
        change: &ObjectChange,
        source: Option<&TableDetail>,
    ) -> Result<ObjectPlan, GenerateError> {
        self.metadata.object_sql(change, source)
    }
    fn connect<'a>(
        &'a self,
        request: &'a ConnectRequest,
        trace: &'a mut ConnectTrace,
    ) -> BoxFuture<'a, Result<Box<dyn Connection>, DriverError>> {
        Box::pin(async move {
            self.open(request, trace)
                .await
                .map(|connection| Box::new(connection) as Box<dyn Connection>)
        })
    }
}
/// One host-owned native connection, exclusively held by its caller/actor.
pub struct HostedConnection {
    pub(crate) duckdb: bool,
    pub(crate) reservation: Option<crate::FileReservation>,
    pub(crate) facets: Facets,
    pub(crate) host: HostSession,
    pub(crate) cancel: Arc<Cancellation>,
    pub(crate) server: ServerSummary,
    pub(crate) transaction: bool,
    pub(crate) failed_connect: Option<openrows_host_protocol::WireTrace>,
    pub(crate) next: u64,
    pub(crate) result: u32,
    pub(crate) metrics: Arc<crate::adapter_metrics::Metrics>,
}
impl HostedConnection {
    /// Authenticated host startup on the parent clock, before database connect.
    #[must_use]
    pub fn handshake_duration(&self) -> std::time::Duration {
        self.host.handshake_duration()
    }
    /// Measured credit and batch high-water marks; no row data is retained.
    #[must_use]
    pub fn metrics(&self) -> crate::HostMetrics {
        self.metrics.snapshot()
    }
    /// PID of this owned process, for bounded lifecycle diagnostics.
    #[must_use]
    pub fn process_id(&self) -> u32 {
        self.host.pid()
    }
    fn new(host: HostSession) -> Self {
        Self {
            duckdb: false,
            reservation: None,
            facets: Facets {
                catalog: false,
                editor: false,
                admin: false,
                bulk: false,
            },
            cancel: Cancellation::new(&host),
            host,
            next: 0,
            result: 0,
            metrics: Arc::default(),
            transaction: false,
            failed_connect: None,
            server: ServerSummary {
                product: String::new(),
                version: String::new(),
                flavour: None,
                database: String::new(),
                user: String::new(),
                tls: TlsStatus::LocalSocket,
            },
        }
    }
    pub(crate) fn protocol(&self) -> DriverError {
        self.host.poison(crate::HostFailure::Protocol);
        DriverError::Host(crate::HostFailure::Protocol)
    }
}
impl Connection for HostedConnection {
    fn server(&self) -> &ServerSummary {
        &self.server
    }
    fn catalog(&mut self) -> Option<&mut dyn openrows_driver_api::Catalog> {
        self.facets.catalog.then_some(self)
    }
    fn editor(&mut self) -> Option<&mut dyn openrows_driver_api::Editor> {
        self.facets.editor.then_some(self)
    }
}
