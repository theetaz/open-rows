// SPDX-License-Identifier: Apache-2.0
//! M2-09 object viewers and read-only scoped account permissions.
use super::support::{TestResult, run, session};
use openrows_driver_api::Catalog;
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::inspection::ObjectIdentity;
use openrows_driver_mssql::SqlServerSession;

const FIXTURE: &[&str] = &[
    "CREATE SCHEMA m209",
    "CREATE TABLE m209.items (id int PRIMARY KEY, label nvarchar(40))",
    "CREATE VIEW m209.labels AS SELECT label FROM m209.items",
    "CREATE FUNCTION m209.echo(@value int) RETURNS int AS BEGIN RETURN @value END",
    "CREATE PROCEDURE m209.touch @value int AS SELECT @value",
    "CREATE SEQUENCE m209.ticket AS bigint START WITH 17 INCREMENT BY 3 CACHE 5",
    "CREATE TYPE m209.label FROM nvarchar(40) NOT NULL",
    "CREATE TYPE m209.address AS TABLE (street nvarchar(80), zip int)",
    "CREATE ROLE m209_parent",
    "CREATE USER m209_reader WITHOUT LOGIN",
    "ALTER ROLE m209_parent ADD MEMBER m209_reader",
    "GRANT SELECT ON m209.items TO m209_reader",
];

const CLEANUP: &[&str] = &[
    "DROP USER m209_reader",
    "DROP ROLE m209_parent",
    "DROP VIEW m209.labels",
    "DROP FUNCTION m209.echo",
    "DROP PROCEDURE m209.touch",
    "DROP SEQUENCE m209.ticket",
    "DROP TYPE m209.address",
    "DROP TYPE m209.label",
    "DROP TABLE m209.items",
    "DROP SCHEMA m209",
];

#[tokio::test]
#[ignore = "needs SQL Server: run `just test-mssql`"]
async fn inspect_objects_types_and_scoped_principal_grants() -> TestResult {
    let mut session = session().await?;
    run(&mut session, FIXTURE).await?;
    let outcome = check(&mut session).await;
    run(&mut session, CLEANUP).await?;
    outcome
}

async fn check(session: &mut SqlServerSession) -> TestResult {
    let objects = session.list_objects("m209", &ObjectKind::ALL).await?;
    for (name, kind) in [
        ("labels", ObjectKind::View),
        ("echo", ObjectKind::Function),
        ("touch", ObjectKind::Procedure),
        ("ticket", ObjectKind::Sequence),
        ("label", ObjectKind::Type),
        ("address", ObjectKind::Type),
    ] {
        assert!(
            objects
                .iter()
                .any(|object| object.name == name && object.kind == kind)
        );
        let object = ObjectIdentity {
            schema: "m209".to_owned(),
            name: name.to_owned(),
            kind,
            arguments: None,
        };
        let inspected = session.object_detail(&object).await?;
        assert!(!inspected.attributes.is_empty() || !inspected.sections.is_empty());
        if matches!(
            kind,
            ObjectKind::View | ObjectKind::Function | ObjectKind::Procedure
        ) {
            assert!(inspected.definition.is_some());
        }
        if name == "address" {
            assert_eq!(inspected.sections[0].rows.len(), 2);
        }
        if name == "label" {
            assert!(
                inspected
                    .attributes
                    .iter()
                    .any(|attribute| attribute.value.as_deref() == Some("nvarchar(40)"))
            );
        }
    }
    let index = session.search_index().await?;
    assert!(index.iter().any(|object| object.schema == "m209"
        && object.name == "address"
        && object.kind == ObjectKind::Type));
    accounts(session).await
}

async fn accounts(session: &mut SqlServerSession) -> TestResult {
    let roles = session.list_roles().await?;
    assert!(
        roles
            .iter()
            .any(|role| role.name == "sa" && role.scope == "server")
    );
    let role = roles
        .iter()
        .find(|role| role.name == "m209_reader" && role.scope == "database")
        .ok_or("database user missing")?;
    assert_eq!(role.memberships, ["m209_parent"]);
    let grants = session.role_grants((&role.name, &role.scope)).await?;
    assert!(grants.iter().any(|grant| grant.object == "[m209].[items]"
        && grant.privilege == "SELECT"
        && grant.state == "GRANT"));
    Ok(())
}
