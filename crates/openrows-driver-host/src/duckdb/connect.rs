// SPDX-License-Identifier: Apache-2.0
//! File modes and locked engine policy are set before any user SQL.
use super::{error, session::DuckdbSession};
use duckdb::{AccessMode, Config};
use openrows_driver_api::{
    ConnectRequest, ConnectStage, ConnectTrace, Connection, DriverError, Endpoint, FileSource,
    TlsStatus,
};

pub(crate) async fn connect(
    request: &ConnectRequest,
    trace: &mut ConnectTrace,
) -> Result<Box<dyn Connection>, DriverError> {
    let Endpoint::File { source, attached } = &request.endpoint else {
        return Err(DriverError::Unsupported(
            "DuckDB opens a file or :memory:".into(),
        ));
    };
    if request.via.is_some() || !attached.is_empty() {
        return Err(DriverError::Unsupported(
            "DuckDB routes and attachments are not supported".into(),
        ));
    }
    let native = trace
        .stage(ConnectStage::Connect, async { open(source) })
        .await?;
    trace.tls = Some(TlsStatus::LocalSocket);
    let mut session = DuckdbSession::new(native, source)?;
    trace
        .stage(ConnectStage::StartupCommands, async {
            for (index, sql) in request.startup_commands.iter().enumerate() {
                session
                    .run(sql, &[], &mut StartupSink)
                    .map_err(|e| e.in_startup_command(index))?;
            }
            Ok(())
        })
        .await?;
    Ok(Box::new(session))
}
fn config(read_only: bool) -> Result<Config, DriverError> {
    let mode = if read_only {
        AccessMode::ReadOnly
    } else {
        AccessMode::ReadWrite
    };
    let config = Config::default().access_mode(mode).map_err(error::open)?;
    let settings = [
        ("memory_limit", "128MiB"),
        ("threads", "2"),
        ("autoload_known_extensions", "false"),
        ("autoinstall_known_extensions", "false"),
        ("allow_persistent_secrets", "false"),
        ("allow_community_extensions", "false"),
        ("enable_external_access", "false"),
        ("arrow_lossless_conversion", "true"),
    ];
    settings
        .into_iter()
        .try_fold(config, |config, (key, value)| {
            config.with(key, value).map_err(error::open)
        })
}
fn open(source: &FileSource) -> Result<duckdb::Connection, DriverError> {
    let config = config(source.read_only())?;
    let native = match source {
        FileSource::Memory { .. } => duckdb::Connection::open_in_memory_with_flags(config),
        FileSource::Disk {
            path,
            create,
            read_only,
        } => {
            if (!create || *read_only) && !path.is_file() {
                return Err(DriverError::Server {
                    code: "58030".into(),
                    message: "DuckDB file does not exist; choose New file to create it".into(),
                });
            }
            duckdb::Connection::open_with_flags(path, config)
        }
    }
    .map_err(error::open)?;
    native
        .execute_batch("SET lock_configuration = true")
        .map_err(error::open)?;
    Ok(native)
}

// Startup SQL must drain every statement, even when an earlier one returns rows.
struct StartupSink;
impl openrows_driver_api::ResultSink for StartupSink {
    fn begin_result(&mut self, _: arrow_schema::SchemaRef) {}
    fn push_batch(&mut self, _: arrow_array::RecordBatch) -> openrows_driver_api::SinkFlow {
        openrows_driver_api::SinkFlow::Continue
    }
}
