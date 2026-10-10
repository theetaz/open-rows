// SPDX-License-Identifier: Apache-2.0
//! The conformance suite on a temporary SQLite-compatible libSQL file (runs in `just test`).
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, Endpoint, FileSource, SslMode, TlsFiles,
};
use openrows_driver_conformance::{Failure, Target, conformance_tests};
use openrows_driver_sqlite::LibsqlFileDriver;
use tempfile::TempDir;

fn prepare() -> Result<(Target, TempDir), Failure> {
    let dir = tempfile::tempdir().map_err(|error| Failure::Setup(error.to_string()))?;
    let request = ConnectRequest {
        endpoint: Endpoint::File {
            source: FileSource::Disk {
                path: dir.path().join("conformance-libsql.db"),
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
        driver: Box::new(LibsqlFileDriver),
        request,
        schema: "main".to_owned(),
    };
    Ok((target, dir))
}

conformance_tests!(prepare);
