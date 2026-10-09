// SPDX-License-Identifier: Apache-2.0
//! The conformance suite against the container `just test-clickhouse` starts
//! (ignored in the default run).
use std::time::Duration;

use openrows_driver_api::{ConnectRequest, ConnectTimeouts, Endpoint, SslMode, TlsFiles};
use openrows_driver_clickhouse::ClickHouseDriver;
use openrows_driver_conformance::{Failure, Target, conformance_tests};
use openrows_net::SecretString;

/// The database `scripts/test-clickhouse.sh` creates.
const DATABASE: &str = "openrows_test";

fn variable(name: &str) -> Result<String, Failure> {
    std::env::var(name)
        .map_err(|_| Failure::Setup(format!("{name} is unset; run `just test-clickhouse`")))
}

fn prepare() -> Result<(Target, ()), Failure> {
    let port = variable("OPENROWS_TEST_CLICKHOUSE_PORT")?
        .parse()
        .map_err(|_| Failure::Setup("OPENROWS_TEST_CLICKHOUSE_PORT is not a port".to_owned()))?;
    let request = ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port,
        },
        database: DATABASE.to_owned(),
        user: "default".to_owned(),
        password: Some(SecretString::from(
            variable("OPENROWS_TEST_CLICKHOUSE_PASSWORD")?.as_str(),
        )),
        application_name: "openrows-conformance".to_owned(),
        // The container's certificate is self-signed.
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        // The server may run emulated (amd64 on arm64): allow slow logins.
        timeouts: ConnectTimeouts {
            auth: Duration::from_secs(60),
            ..ConnectTimeouts::default()
        },
        startup_commands: Vec::new(),
    };
    let target = Target {
        driver: Box::new(ClickHouseDriver),
        request,
        schema: DATABASE.to_owned(),
    };
    Ok((target, ()))
}

conformance_tests!(
    #[ignore = "needs ClickHouse; run with `just test-clickhouse`"]
    prepare
);
