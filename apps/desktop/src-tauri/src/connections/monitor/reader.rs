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

/// The collector for `target`: `DuckDB` borrows its owning shared session,
/// every other engine opens its own monitoring connection.
pub(super) async fn open_collector(
    state: &Connections,
    target: &super::MonitorTarget,
) -> Result<Box<dyn MonitorReader>, IpcError> {
    let profile = state
        .profiles
        .get(profiles::profile_id(&target.profile_id)?)?;
    if profile.driver == openrows_store::profile::Driver::Duckdb {
        return crate::connections::duckdb::monitor(state, &target.profile_id).await;
    }
    Ok(Box::new(open(state, target).await?))
}

pub(super) async fn open(
    state: &Connections,
    target: &super::MonitorTarget,
) -> Result<Reader, IpcError> {
    let profile = state
        .profiles
        .get(profiles::profile_id(&target.profile_id)?)?;
    ensure_engine(profile.driver)?;
    Ok(Reader(connect(state, target, "monitor").await?))
}

/// A fresh connection of a connected profile, without its startup SQL.
pub(super) async fn connect(
    state: &Connections,
    target: &super::MonitorTarget,
    suffix: &str,
) -> Result<Box<dyn Connection>, IpcError> {
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
    profile.startup_commands.clear();
    let route = route::profile_target_for(state, &target.profile_id, profile)
        .await?
        .with_database(&target.database)
        .with_application_suffix(suffix);
    let connection = target::connect(&route).await.map_err(|_| {
        IpcError::new(
            IpcErrorKind::Network,
            "Monitoring session could not connect",
        )
    })?;
    Ok(connection.connection)
}

pub(super) fn ensure_engine(engine: openrows_store::profile::Driver) -> Result<(), IpcError> {
    use openrows_store::profile::Driver;
    let reason = match engine {
        Driver::Postgres
        | Driver::Mysql
        | Driver::Mariadb
        | Driver::Sqlserver
        | Driver::Clickhouse
        | Driver::Sqlite
        | Driver::LibsqlFile
        | Driver::Libsql => return Ok(()),
        Driver::Duckdb => {
            "DuckDB diagnostics run only on its owning session; no extra connection is opened"
        }
    };
    Err(IpcError::new(IpcErrorKind::Unsupported, reason))
}

/// Monitor Explain and `HypoPG` open a `PostgreSQL` diagnostic session; refuse other engines.
pub(super) fn ensure_postgres(engine: openrows_store::profile::Driver) -> Result<(), IpcError> {
    if engine == openrows_store::profile::Driver::Postgres {
        return Ok(());
    }
    Err(IpcError::new(
        IpcErrorKind::Unsupported,
        "Monitor diagnostics are PostgreSQL only; use the editor's Explain for this engine",
    ))
}

#[cfg(test)]
mod tests {
    use super::{IpcErrorKind, ensure_postgres};
    use openrows_store::profile::Driver;

    #[test]
    fn monitor_diagnostics_refuse_every_engine_but_postgres() {
        assert!(ensure_postgres(Driver::Postgres).is_ok());
        for driver in [
            Driver::Mysql,
            Driver::Mariadb,
            Driver::Sqlserver,
            Driver::Clickhouse,
            Driver::Sqlite,
            Driver::LibsqlFile,
            Driver::Libsql,
            Driver::Duckdb,
        ] {
            let refused = ensure_postgres(driver).err().map(|error| error.kind);
            assert_eq!(refused, Some(IpcErrorKind::Unsupported));
        }
    }
}
