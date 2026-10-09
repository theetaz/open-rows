// SPDX-License-Identifier: Apache-2.0
//! Public libpq service import and peer/trust passwordless authentication.
use super::support::{TestResult, config};
use openrows_driver_postgres::{PgConfig, PgSession};
use openrows_store::profile::{PasswordStorage, tool_import};
#[tokio::test]
#[ignore = "needs PostgreSQL; run with just test-pg"]
async fn pg_service_import_connects() -> TestResult {
    let config = config()?;
    let fixture = format!(
        "[imported-service]\nhost={}\nport={}\ndbname={}\nuser={}\nsslmode=disable\napplication_name=OpenRows\nconnect_timeout=10\n",
        config.host, config.port, config.database, config.user
    );
    let batch = tool_import::parse_pg_service(&fixture)?;
    let profile = &batch.entries[0].profile;
    let parsed = PgConfig {
        host: profile.params.host.clone(),
        port: profile.params.port,
        user: profile.params.user.clone().ok_or("user")?,
        password: config.password,
        database: profile.params.database.clone().ok_or("database")?,
    };
    let _session = PgSession::connect(&parsed).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "needs PostgreSQL; run with just test-pg"]
async fn none_policy_connects_with_passwordless_trust_authentication() -> TestResult {
    let container = std::env::var("OPENROWS_TEST_PG_CONTAINER")?;
    let output = std::process::Command::new("docker").args(["exec", &container, "sh", "-c", "printf 'host all openrows_none 0.0.0.0/0 trust\n' | cat - /var/lib/postgresql/data/pg_hba.conf > /tmp/openrows-hba && cat /tmp/openrows-hba > /var/lib/postgresql/data/pg_hba.conf && rm /tmp/openrows-hba"]).output()?;
    if !output.status.success() {
        return Err("cannot configure isolated trust role".into());
    }
    let mut admin = super::support::connect().await?;
    for sql in ["CREATE ROLE openrows_none LOGIN", "SELECT pg_reload_conf()"] {
        super::support::run(&mut admin, sql, super::support::Collect::default())
            .await
            .0?;
    }
    let batch = tool_import::parse_pg_service(&format!(
        "[none]\nhost=127.0.0.1\nport={}\ndbname=postgres\nuser=openrows_none\nsslmode=disable",
        config()?.port
    ))?;
    let mut profile = batch.entries.into_iter().next().ok_or("profile")?.profile;
    profile.password_storage = PasswordStorage::None;
    assert!(!profile.uses_secret(openrows_store::profile::SecretSlot::Password));
    let _session = PgSession::connect(&PgConfig {
        host: profile.params.host,
        port: profile.params.port,
        user: "openrows_none".into(),
        password: None,
        database: "postgres".into(),
    })
    .await?;
    Ok(())
}
