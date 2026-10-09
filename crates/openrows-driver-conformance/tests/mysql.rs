// SPDX-License-Identifier: Apache-2.0
//! The conformance suite against the containers `just test-mysql` starts
//! (ignored in the default run).
use openrows_driver_api::{ConnectRequest, ConnectTimeouts, Endpoint, SslMode, TlsFiles};
use openrows_driver_conformance::{Failure, Target, conformance_tests};
use openrows_driver_mysql::{Flavour, MySqlDriver};
use openrows_net::SecretString;

/// The database `scripts/test-mysql.sh` creates.
const DATABASE: &str = "openrows_test";

fn variable(name: &str) -> Result<String, Failure> {
    std::env::var(name)
        .map_err(|_| Failure::Setup(format!("{name} is unset; run `just test-mysql`")))
}

fn prepare() -> Result<(Target, ()), Failure> {
    let port = variable("OPENROWS_TEST_MYSQL_PORT")?
        .parse()
        .map_err(|_| Failure::Setup("OPENROWS_TEST_MYSQL_PORT is not a port".to_owned()))?;
    let flavour = match variable("OPENROWS_TEST_MYSQL_ENGINE")?.as_str() {
        "mariadb" => Flavour::MariaDb,
        _ => Flavour::MySql,
    };
    let request = ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port,
        },
        database: DATABASE.to_owned(),
        user: "root".to_owned(),
        password: Some(SecretString::from(
            variable("OPENROWS_TEST_MYSQL_PASSWORD")?.as_str(),
        )),
        application_name: "openrows-conformance".to_owned(),
        ssl_mode: SslMode::Prefer,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
    };
    let target = Target {
        driver: Box::new(MySqlDriver(flavour)),
        request,
        schema: DATABASE.to_owned(),
    };
    Ok((target, ()))
}

conformance_tests!(
    #[ignore = "needs MySQL or MariaDB; run with `just test-mysql`"]
    prepare
);
