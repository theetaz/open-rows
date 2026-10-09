// SPDX-License-Identifier: Apache-2.0
//! A login can read public account flags without access to role system tables.
use super::support::{TestResult, mariadb, request, run, session};
use openrows_driver_api::{Catalog, ConnectTrace, SslMode};
use openrows_driver_mysql::connect;
use openrows_net::SecretString;

#[tokio::test]
#[ignore = "needs MySQL/MariaDB: run `just test-mysql`"]
async fn unreadable_memberships_do_not_fail_the_account_or_grants_viewer() -> TestResult {
    let mut root = session().await?;
    run(
        &mut root,
        &[
            "CREATE USER 'm209_viewer'@'%' IDENTIFIED BY 'pw'",
            "GRANT SELECT ON mysql.user TO 'm209_viewer'@'%'",
        ],
    )
    .await?;
    if mariadb() {
        run(
            &mut root,
            &["GRANT SELECT ON mysql.global_priv TO 'm209_viewer'@'%'"],
        )
        .await?;
    }
    let outcome = check().await;
    run(&mut root, &["DROP USER 'm209_viewer'@'%'"]).await?;
    outcome
}

async fn check() -> TestResult {
    let mut request = request(SslMode::Prefer)?;
    request.database = String::new();
    "m209_viewer".clone_into(&mut request.user);
    request.password = Some(SecretString::from("pw"));
    let mut session = connect(&request, &mut ConnectTrace::default()).await?;
    let roles = session.list_roles().await?;
    let viewer = roles
        .iter()
        .find(|role| role.name == "m209_viewer")
        .ok_or("viewer missing")?;
    assert!(viewer.memberships.is_empty());
    assert!(
        viewer
            .attributes
            .iter()
            .any(|attribute| attribute.name == "Membership visibility"
                && attribute.value.as_deref() == Some("Not visible to this login"))
    );
    let grants = session.role_grants((&viewer.name, &viewer.scope)).await?;
    assert!(
        grants
            .iter()
            .any(|grant| grant.object == "`mysql`.`user`" && grant.privilege == "SELECT")
    );
    Ok(())
}
