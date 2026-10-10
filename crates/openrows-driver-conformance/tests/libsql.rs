// SPDX-License-Identifier: Apache-2.0
//! Remote libSQL capability conformance on the authenticated pinned sqld image.
use openrows_driver_api::{ConnectRequest, ConnectTimeouts, Endpoint, SslMode, TlsFiles};
use openrows_driver_conformance::{Failure, Target, conformance_tests};
use openrows_driver_libsql::LibsqlDriver;
use openrows_net::SecretString;
fn variable(name: &str) -> Result<String, Failure> {
    std::env::var(name)
        .map_err(|_| Failure::Setup(format!("{name} is unset; run just test-libsql")))
}
fn prepare() -> Result<(Target, ()), Failure> {
    let port = variable("OPENROWS_TEST_LIBSQL_PORT")?
        .parse()
        .map_err(|_| Failure::Setup("invalid libSQL test port".into()))?;
    let request = ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".into(),
            port,
        },
        database: "main".into(),
        user: String::new(),
        password: Some(SecretString::from(
            variable("OPENROWS_TEST_LIBSQL_TOKEN")?.as_str(),
        )),
        application_name: "openrows-conformance".into(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        driver_options: std::collections::BTreeMap::new(),
        startup_commands: Vec::new(),
    };
    Ok((
        Target {
            driver: Box::new(LibsqlDriver),
            request,
            schema: "main".into(),
        },
        (),
    ))
}
conformance_tests!(
    #[ignore = "needs authenticated sqld; run just test-libsql"]
    prepare
);
