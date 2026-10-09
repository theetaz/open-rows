// SPDX-License-Identifier: Apache-2.0
//! `MySQL` option-file imports connect to the integration suite's isolated server.
use super::support;
use openrows_driver_api::ConnectTrace;
use openrows_store::profile::tool_import;
#[tokio::test]
#[ignore = "needs MySQL/MariaDB; run with just test-mysql"]
async fn my_cnf_import_connects() -> Result<(), Box<dyn std::error::Error>> {
    let base = support::request(openrows_driver_api::SslMode::Disable)?;
    let port = match base.endpoint {
        openrows_driver_api::Endpoint::Server { port, .. } => port,
        openrows_driver_api::Endpoint::File { .. } => return Err("server endpoint required".into()),
    };
    let batch = tool_import::parse_my_cnf(&format!(
        "[client]\nhost=127.0.0.1\nport={port}\nuser=root\ndatabase={}\nssl-mode=DISABLED",
        support::DATABASE
    ))?;
    let profile = &batch.entries[0].profile;
    let request = openrows_driver_api::ConnectRequest {
        endpoint: openrows_driver_api::Endpoint::Server {
            host: profile.params.host.clone(),
            port: profile.params.port,
        },
        database: profile.params.database.clone().ok_or("database")?,
        user: profile.params.user.clone().ok_or("user")?,
        ..base
    };
    let _session = openrows_driver_mysql::connect(&request, &mut ConnectTrace::default()).await?;
    Ok(())
}
