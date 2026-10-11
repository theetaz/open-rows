// SPDX-License-Identifier: Apache-2.0
//! Route reuse with arbitrary profile startup SQL excluded before target validation.
use crate::connections::{Connections, IpcError, IpcErrorKind, profiles, route, target};
use openrows_driver_api::Connection;
use openrows_monitor::{Descriptor, MonitorReader, ReadFuture, Reading, SampleState, Source};

pub(super) struct Reader(pub(super) Box<dyn Connection>);
impl MonitorReader for Reader {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        match self.0.monitor() {
            Some(reader) => reader.discover(),
            None => Box::pin(async { Vec::new() }),
        }
    }
    fn read(&mut self, source: Source, deadline: u32) -> ReadFuture<'_, Reading> {
        match self.0.monitor() {
            Some(reader) => reader.read(source, deadline),
            None => Box::pin(async {
                Reading::missing(SampleState::Unavailable("Collector is unsupported".into()))
            }),
        }
    }
    fn cancellation(&self) -> ReadFuture<'static, ()> {
        let cancel = self.0.cancel_handle();
        Box::pin(async move {
            if let Some(cancel) = cancel {
                let _ = cancel.cancel().await;
            }
        })
    }
}

pub(super) async fn open(
    state: &Connections,
    target: &super::MonitorTarget,
) -> Result<Reader, IpcError> {
    let mut profile = state
        .profiles
        .get(profiles::profile_id(&target.profile_id)?)?;
    if !state.metadata.profiles().contains(&target.profile_id)
        && !state
            .owners
            .all()
            .iter()
            .any(|(_, owner)| owner.profile == profile.id)
    {
        return Err(IpcError::invalid(
            "Connect to this profile before explicitly starting monitoring",
        ));
    }
    ensure_engine(profile.driver)?;
    profile.startup_commands.clear();
    let route = route::profile_target_for(state, &target.profile_id, profile)
        .await?
        .with_database(&target.database)
        .with_application_suffix("monitor");
    let connection = target::connect(&route).await.map_err(|_| {
        IpcError::new(
            IpcErrorKind::Network,
            "Monitoring session could not connect",
        )
    })?;
    Ok(Reader(connection.connection))
}

pub(super) fn ensure_engine(engine: openrows_store::profile::Driver) -> Result<(), IpcError> {
    use openrows_store::profile::Driver;
    let reason = match engine {
        Driver::Postgres => return Ok(()),
        Driver::Duckdb => {
            "DuckDB monitoring requires its owning host/session; no extra connection is opened"
        }
        Driver::Sqlite | Driver::LibsqlFile => {
            "Own-session embedded diagnostics are not implemented yet"
        }
        Driver::Libsql => "Own OpenRows requests only; server-wide metrics unavailable over Hrana",
        Driver::Mysql | Driver::Mariadb | Driver::Sqlserver | Driver::Clickhouse => {
            "This engine's monitoring collector is not implemented yet"
        }
    };
    Err(IpcError::new(IpcErrorKind::Unsupported, reason))
}
