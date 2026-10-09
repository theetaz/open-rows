// SPDX-License-Identifier: Apache-2.0
//! M2-09 listings, object inspection, memberships and non-mutating sequences.
use super::support::{Collect, TestResult, connect, run, single_batch};
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::inspection::{ObjectDetail, ObjectIdentity};
use openrows_driver_api::{Catalog, DriverError};
use openrows_driver_postgres::PgSession;

const FIXTURE: &[&str] = &[
    "CREATE SCHEMA m209",
    "CREATE TABLE m209.items (id int PRIMARY KEY, label text)",
    "CREATE VIEW m209.labels AS SELECT label FROM m209.items",
    "CREATE MATERIALIZED VIEW m209.totals AS SELECT count(*) AS total FROM m209.items WITH NO DATA",
    "CREATE SEQUENCE m209.ticket START 17 INCREMENT 3 CACHE 5",
    "CREATE FUNCTION m209.echo(value text) RETURNS text LANGUAGE sql IMMUTABLE SECURITY DEFINER AS 'SELECT value'",
    "CREATE FUNCTION m209.echo(value integer) RETURNS integer LANGUAGE sql STABLE AS 'SELECT value'",
    "CREATE PROCEDURE m209.touch() LANGUAGE sql AS 'SELECT 1'",
    "CREATE TYPE m209.status AS ENUM ('pending', 'done', 'a,b')",
    "CREATE TYPE m209.address AS (street text, zip integer)",
    "CREATE DOMAIN m209.positive AS integer CHECK (VALUE > 0)",
    "CREATE TYPE m209.span AS RANGE (SUBTYPE = integer)",
    "CREATE EXTENSION hstore WITH SCHEMA m209",
    "CREATE ROLE m209_parent NOLOGIN",
    "CREATE ROLE m209_reader LOGIN CONNECTION LIMIT 7",
    "GRANT m209_parent TO m209_reader",
    "GRANT SELECT ON m209.items TO m209_reader",
];

#[tokio::test]
#[ignore = "needs PostgreSQL: run `just test-pg`"]
async fn inspect_all_objects_roles_and_extensions_without_advancing_sequences() -> TestResult {
    let mut session = connect().await?;
    for sql in FIXTURE {
        run(&mut session, sql, Collect::default()).await.0?;
    }
    let result = check(&mut session).await;
    for sql in [
        "DROP SCHEMA m209 CASCADE",
        "DROP ROLE m209_reader",
        "DROP ROLE m209_parent",
    ] {
        run(&mut session, sql, Collect::default()).await.0?;
    }
    result
}

async fn detail(
    session: &mut PgSession,
    name: &str,
    kind: ObjectKind,
) -> Result<ObjectDetail, DriverError> {
    session
        .object_detail(&ObjectIdentity {
            schema: "m209".to_owned(),
            name: name.to_owned(),
            kind,
            arguments: None,
        })
        .await
}

fn value<'a>(detail: &'a ObjectDetail, name: &str) -> Option<&'a str> {
    detail
        .attributes
        .iter()
        .find(|attribute| attribute.name == name)
        .and_then(|attribute| attribute.value.as_deref())
}

async fn check(session: &mut PgSession) -> TestResult {
    let objects = session.list_objects("m209", &ObjectKind::ALL).await?;
    for (name, kind) in [
        ("labels", ObjectKind::View),
        ("totals", ObjectKind::MaterializedView),
        ("echo", ObjectKind::Function),
        ("touch", ObjectKind::Procedure),
        ("ticket", ObjectKind::Sequence),
        ("status", ObjectKind::Type),
        ("address", ObjectKind::Type),
        ("positive", ObjectKind::Type),
        ("span", ObjectKind::Type),
    ] {
        assert!(
            objects
                .iter()
                .any(|object| object.name == name && object.kind == kind),
            "{name}: {objects:?}"
        );
        let inspected = detail(session, name, kind).await?;
        assert!(!inspected.attributes.is_empty() || !inspected.sections.is_empty());
    }
    let index = session.search_index().await?;
    assert!(index.iter().any(|object| object.schema == "m209"
        && object.name == "status"
        && object.kind == ObjectKind::Type));
    let matview = detail(session, "totals", ObjectKind::MaterializedView).await?;
    assert_eq!(value(&matview, "Populated"), Some("false"));
    assert!(
        matview
            .definition
            .as_deref()
            .is_some_and(|sql| sql.contains("WITH NO DATA"))
    );
    let enum_type = detail(session, "status", ObjectKind::Type).await?;
    assert_eq!(
        enum_type.sections[0]
            .rows
            .iter()
            .map(|row| row[0].as_deref())
            .collect::<Vec<_>>(),
        [Some("pending"), Some("done"), Some("a,b")]
    );
    let domain = detail(session, "positive", ObjectKind::Type).await?;
    assert_eq!(value(&domain, "Base type"), Some("integer"));
    assert!(
        domain
            .sections
            .iter()
            .any(|section| section.name == "Domain constraints" && section.rows.len() == 1)
    );
    overload(session).await?;
    sequence(session).await?;
    accounts(session).await
}

async fn overload(session: &mut PgSession) -> TestResult {
    let object = ObjectIdentity {
        schema: "m209".to_owned(),
        name: "echo".to_owned(),
        kind: ObjectKind::Function,
        arguments: Some("value text".to_owned()),
    };
    let inspected = session.object_detail(&object).await?;
    assert_eq!(value(&inspected, "Return type"), Some("text"));
    assert_eq!(value(&inspected, "Volatility"), Some("immutable"));
    assert_eq!(value(&inspected, "Security definer"), Some("true"));
    assert!(
        inspected
            .definition
            .as_deref()
            .is_some_and(|sql| sql.contains("value text"))
    );
    Ok(())
}

async fn sequence(session: &mut PgSession) -> TestResult {
    let before = single_batch(session, "SELECT last_value::text FROM m209.ticket").await?;
    let inspected = detail(session, "ticket", ObjectKind::Sequence).await?;
    assert_eq!(value(&inspected, "Start"), Some("17"));
    assert_eq!(value(&inspected, "Increment"), Some("3"));
    assert_eq!(value(&inspected, "Cache"), Some("5"));
    let after = single_batch(session, "SELECT last_value::text FROM m209.ticket").await?;
    assert_eq!(before, after);
    Ok(())
}

async fn accounts(session: &mut PgSession) -> TestResult {
    let extensions = session.list_extensions().await?;
    let extension = extensions
        .iter()
        .find(|extension| extension.name == "hstore")
        .ok_or("hstore missing")?;
    assert_eq!(extension.schema, "m209");
    assert!(extension.available_version.is_some() && extension.description.is_some());
    let roles = session.list_roles().await?;
    let role = roles
        .iter()
        .find(|role| role.name == "m209_reader")
        .ok_or("role missing")?;
    assert_eq!(role.memberships, ["m209_parent"]);
    assert!(
        role.attributes
            .iter()
            .any(|attribute| attribute.name == "Connection limit"
                && attribute.value.as_deref() == Some("7"))
    );
    let grants = session.role_grants((&role.name, &role.scope)).await?;
    assert!(
        grants
            .iter()
            .any(|grant| grant.object == "m209.items" && grant.privilege == "SELECT")
    );
    Ok(())
}
