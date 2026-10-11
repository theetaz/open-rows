// SPDX-License-Identifier: Apache-2.0
//! The conformance suite on a temporary `SQLite` file (runs in `just test`).
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, Endpoint, FileSource, SslMode, TlsFiles,
};
use openrows_driver_conformance::{Failure, Target, conformance_tests};
use openrows_driver_sqlite::SqliteDriver;
use openrows_host_client::{
    CircuitBreaker, HostSupervisor, HostedDriver, LaunchOptions, MemoryBudget,
};
use std::sync::{Arc, Mutex, MutexGuard};
#[path = "support/diagnostics.rs"]
mod diagnostics;
#[cfg(target_os = "macos")]
#[path = "support/image.rs"]
mod image;
fn options(memory: MemoryBudget) -> LaunchOptions {
    LaunchOptions {
        driver: openrows_host_client::HostDriverId::SqliteTest,
        executable: std::path::PathBuf::from(env!("CARGO_BIN_EXE_host-test-fixture")),
        endpoint_root: if cfg!(unix) {
            "/tmp".into()
        } else {
            std::env::temp_dir()
        },
        memory,
        circuit: CircuitBreaker::default(),
    }
}
struct Fixture {
    _directory: TempDir,
    _permit: MutexGuard<'static, ()>,
}

use tempfile::TempDir;

fn prepare() -> Result<(Target, Fixture), Failure> {
    static TEST: Mutex<()> = Mutex::new(());
    diagnostics::init();
    let permit = TEST
        .lock()
        .map_err(|_| Failure::Setup("fixture lock".into()))?;
    #[cfg(target_os = "macos")]
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Failure::Setup(e.to_string()))?
        .block_on(image::prepare())
        .map_err(|e| Failure::Setup(e.to_string()))?;
    let dir = tempfile::tempdir().map_err(|error| Failure::Setup(error.to_string()))?;
    let request = ConnectRequest {
        endpoint: Endpoint::File {
            source: FileSource::Disk {
                path: dir.path().join("conformance.sqlite"),
                read_only: false,
                create: true,
            },
            attached: Vec::new(),
        },
        database: "main".to_owned(),
        user: String::new(),
        password: None,
        application_name: "openrows-conformance".to_owned(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        driver_options: std::collections::BTreeMap::new(),
        startup_commands: Vec::new(),
    };
    let target = Target {
        driver: Box::new(HostedDriver::new(
            HostSupervisor::new(options(MemoryBudget::default())),
            Arc::new(SqliteDriver),
        )),
        request,
        schema: "main".to_owned(),
    };
    Ok((
        target,
        Fixture {
            _directory: dir,
            _permit: permit,
        },
    ))
}

conformance_tests!(prepare);
