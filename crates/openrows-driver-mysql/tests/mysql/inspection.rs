// SPDX-License-Identifier: Apache-2.0
//! Bound view/routine reads and public account attributes on both servers.
use super::support::{DATABASE, TestResult, run, session};
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::inspection::ObjectIdentity;
use openrows_driver_api::{Catalog, DriverError, Session};
use openrows_driver_mysql::MySqlSession;

#[tokio::test]
#[ignore = "needs MySQL/MariaDB: run `just test-mysql`"]
async fn inspect_views_routines_and_public_accounts_on_both_servers() -> TestResult {
    let mut session = session().await?;
    run(
        &mut session,
        &[
            "CREATE VIEW m209_labels AS SELECT 1 AS id",
            "CREATE FUNCTION m209_echo(value integer) RETURNS integer DETERMINISTIC RETURN value",
            "CREATE PROCEDURE m209_touch(IN value integer) SELECT value",
            "CREATE TABLE m209_grants (id int, label varchar(40))",
            "CREATE ROLE m209_parent",
            "CREATE USER 'm209_reader'@'%'",
            "GRANT m209_parent TO 'm209_reader'@'%'",
            "GRANT PROCESS ON *.* TO 'm209_reader'@'%'",
            "GRANT CREATE ON openrows_test.* TO 'm209_reader'@'%'",
            "GRANT SELECT ON openrows_test.m209_grants TO 'm209_reader'@'%' WITH GRANT OPTION",
            "GRANT UPDATE (label) ON openrows_test.m209_grants TO 'm209_reader'@'%'",
        ],
    )
    .await?;
    let outcome = check_objects_and_accounts(&mut session).await;
    run(
        &mut session,
        &[
            "DROP USER 'm209_reader'@'%'",
            "DROP ROLE m209_parent",
            "DROP TABLE m209_grants",
            "DROP VIEW m209_labels",
            "DROP FUNCTION m209_echo",
            "DROP PROCEDURE m209_touch",
        ],
    )
    .await?;
    outcome
}

async fn check_objects_and_accounts(session: &mut MySqlSession) -> TestResult {
    for (name, kind) in [
        ("m209_labels", ObjectKind::View),
        ("m209_echo", ObjectKind::Function),
        ("m209_touch", ObjectKind::Procedure),
    ] {
        let object = ObjectIdentity {
            schema: DATABASE.to_owned(),
            name: name.to_owned(),
            kind,
            arguments: None,
        };
        let detail = session.object_detail(&object).await?;
        assert!(detail.definition.is_some());
        assert!(!detail.sections.is_empty());
    }
    let objects = session.list_objects(DATABASE, &ObjectKind::ALL).await?;
    for (name, kind) in [
        ("m209_labels", ObjectKind::View),
        ("m209_echo", ObjectKind::Function),
        ("m209_touch", ObjectKind::Procedure),
    ] {
        assert!(
            objects
                .iter()
                .any(|object| object.name == name && object.kind == kind)
        );
    }
    let roles = session.list_roles().await?;
    let reader = roles
        .iter()
        .find(|role| role.name == "m209_reader" && role.scope == "%")
        .ok_or("account missing")?;
    assert_eq!(reader.memberships.len(), 1);
    assert!(reader.memberships[0].starts_with("m209_parent"));
    assert!(
        reader
            .attributes
            .iter()
            .any(|attribute| attribute.name == "Password expired")
    );
    check_grants(session).await?;
    assert!(matches!(
        session.list_extensions().await,
        Err(DriverError::Unsupported(_))
    ));
    let unsupported = ObjectIdentity {
        schema: DATABASE.to_owned(),
        name: "x".to_owned(),
        kind: ObjectKind::Type,
        arguments: None,
    };
    assert!(matches!(
        session.object_detail(&unsupported).await,
        Err(DriverError::Unsupported(_))
    ));
    Ok(())
}

async fn check_grants(session: &mut MySqlSession) -> TestResult {
    if !super::support::mariadb() {
        run(
            session,
            &["GRANT BACKUP_ADMIN ON *.* TO 'm209_reader'@'%' WITH GRANT OPTION"],
        )
        .await?;
    }
    let grants = session.role_grants(("m209_reader", "%")).await?;
    for (scope, privilege, object, state) in [
        ("global", "PROCESS", "*.*", "GRANT"),
        ("schema", "CREATE", "`openrows_test`.*", "GRANT"),
        (
            "table",
            "SELECT",
            "`openrows_test`.`m209_grants`",
            "GRANT WITH GRANT OPTION",
        ),
        (
            "column",
            "UPDATE",
            "`openrows_test`.`m209_grants`.`label`",
            "GRANT WITH GRANT OPTION",
        ),
    ] {
        assert!(
            grants.iter().any(|grant| grant.scope == scope
                && grant.privilege == privilege
                && grant.object == object
                && grant.state == state
                && grant.grantor.is_none()),
            "{grants:?}"
        );
    }
    if !super::support::mariadb() {
        assert!(
            grants.iter().any(|grant| grant.privilege == "BACKUP_ADMIN"
                && grant.state == "GRANT WITH GRANT OPTION")
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs MySQL/MariaDB: run `just test-mysql`"]
async fn sequence_details_twice_leave_nextval_unchanged_on_mariadb() -> TestResult {
    let mut session = session().await?;
    let object = ObjectIdentity {
        schema: DATABASE.to_owned(),
        name: "m209_ticket".to_owned(),
        kind: ObjectKind::Sequence,
        arguments: None,
    };
    if !super::support::mariadb() {
        assert!(matches!(
            session.object_detail(&object).await,
            Err(DriverError::Unsupported(_))
        ));
        return Ok(());
    }
    run(&mut session, &[
        "CREATE SEQUENCE m209_ticket START WITH 17 INCREMENT BY 3 MINVALUE 1 MAXVALUE 1000 CACHE 5 CYCLE",
        "CREATE SEQUENCE m209_control START WITH 17 INCREMENT BY 3 MINVALUE 1 MAXVALUE 1000 CACHE 5 CYCLE",
    ]).await?;
    let outcome = check_sequence(&mut session, &object).await;
    run(
        &mut session,
        &["DROP SEQUENCE m209_ticket", "DROP SEQUENCE m209_control"],
    )
    .await?;
    outcome
}

async fn check_sequence(session: &mut MySqlSession, object: &ObjectIdentity) -> TestResult {
    let listed = session
        .list_objects(DATABASE, &[ObjectKind::Sequence])
        .await?;
    assert!(listed.iter().any(|object| object.name == "m209_ticket"));
    let first = session.object_detail(object).await?;
    assert_eq!(session.object_detail(object).await?, first);
    for (label, expected) in [
        ("Start", "17"),
        ("Increment", "3"),
        ("Minimum", "1"),
        ("Maximum", "1000"),
        ("Cache", "5"),
        ("Cycle", "1"),
        ("Next uncached value", "17"),
    ] {
        assert!(
            first
                .attributes
                .iter()
                .any(|attribute| attribute.name == label
                    && attribute.value.as_deref() == Some(expected)),
            "{first:?}"
        );
    }
    assert!(
        first
            .attributes
            .iter()
            .any(|attribute| attribute.name == "Data type" && attribute.value.is_some())
    );
    assert!(
        !first
            .attributes
            .iter()
            .any(|attribute| attribute.name == "Last value")
    );
    let mut values = super::support::Collect::default();
    session
        .execute(
            "SELECT NEXTVAL(m209_ticket), NEXTVAL(m209_control)",
            &mut values,
        )
        .await?;
    let batch = values.batches.first().ok_or("missing NEXTVAL result")?;
    assert_eq!(batch.column(0), batch.column(1));
    let value = batch
        .column(0)
        .as_any()
        .downcast_ref::<arrow_array::Int64Array>()
        .ok_or("nextval type")?;
    assert_eq!(value.value(0), 17);
    Ok(())
}
