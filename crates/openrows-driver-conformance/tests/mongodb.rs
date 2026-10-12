// SPDX-License-Identifier: Apache-2.0
//! Capability-driven `MongoDB` subset uses native document contract cases.
use openrows_driver_api::{ConnectRequest, ConnectTimeouts, Endpoint, SslMode, TlsFiles};
use openrows_driver_conformance::{Failure, Target, conformance_tests};
use openrows_driver_mongodb::MongoDriver;
use std::collections::BTreeMap;
fn prepare() -> Result<(Target, ()), Failure> {
    let port = std::env::var("OPENROWS_TEST_MONGO_PORT")
        .map_err(|_| Failure::Setup("run just test-mongo".into()))?
        .parse()
        .map_err(|_| Failure::Setup("invalid MongoDB port".into()))?;
    let request = ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".into(),
            port,
        },
        database: "openrows_conformance".into(),
        user: String::new(),
        password: None,
        application_name: "openrows-conformance".into(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
        driver_options: BTreeMap::from([("mongodb_direct".into(), "true".into())]),
    };
    Ok((
        Target {
            driver: Box::new(MongoDriver),
            request,
            schema: "openrows_conformance".into(),
        },
        (),
    ))
}
conformance_tests!(
    #[ignore = "needs MongoDB replica set; run just test-mongo"]
    prepare
);
