// SPDX-License-Identifier: Apache-2.0
//! The conformance suite against the container `just test-pg` starts
//! (ignored in the default run).
use openrows_driver_api::{ConnectRequest, ConnectTimeouts, Endpoint, SslMode, TlsFiles};
use openrows_driver_conformance::{Failure, Target, conformance_tests};
use openrows_driver_postgres::PgDriver;
use openrows_net::SecretString;

fn variable(name: &str) -> Result<String, Failure> {
    std::env::var(name).map_err(|_| Failure::Setup(format!("{name} is unset; run `just test-pg`")))
}

fn prepare() -> Result<(Target, ()), Failure> {
    let port = variable("OPENROWS_TEST_PG_PORT")?
        .parse()
        .map_err(|_| Failure::Setup("OPENROWS_TEST_PG_PORT is not a port".to_owned()))?;
    let request = ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port,
        },
        database: "postgres".to_owned(),
        user: "postgres".to_owned(),
        password: Some(SecretString::from(
            variable("OPENROWS_TEST_PG_PASSWORD")?.as_str(),
        )),
        application_name: "openrows-conformance".to_owned(),
        ssl_mode: SslMode::Prefer,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        driver_options: std::collections::BTreeMap::new(),
        startup_commands: Vec::new(),
    };
    let target = Target {
        driver: Box::new(PgDriver),
        request,
        schema: "public".to_owned(),
    };
    Ok((target, ()))
}

conformance_tests!(
    #[ignore = "needs PostgreSQL; run with `just test-pg`"]
    prepare
);
