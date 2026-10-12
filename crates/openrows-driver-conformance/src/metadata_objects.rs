// SPDX-License-Identifier: Apache-2.0
//! Additional feature-gated Metadata fixtures for M2-09.
use crate::sink::query;
use crate::{Failure, Group, Target, ensure, fixture_name};
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::inspection::ObjectIdentity;
use openrows_driver_api::{Connection, DriverError, Feature, catalog};
use openrows_sql::Dialect;

struct Fixture {
    kind: ObjectKind,
    name: String,
    create: String,
    drop: String,
}

pub(crate) async fn run(target: &Target, connection: &mut dyn Connection) -> Result<(), Failure> {
    let fixtures = fixtures(target, connection.server().product.as_str())?;
    for fixture in fixtures {
        query(connection, &fixture.create).await?;
        let outcome = check(target, connection, &fixture).await;
        query(connection, &fixture.drop).await?;
        outcome?;
    }
    optional_facets(target, connection).await
}

fn fixtures(target: &Target, product: &str) -> Result<Vec<Fixture>, Failure> {
    let caps = target.driver.capabilities();
    let mut fixtures = Vec::new();
    for kind in [
        ObjectKind::MaterializedView,
        ObjectKind::Function,
        ObjectKind::Procedure,
        ObjectKind::Sequence,
        ObjectKind::Type,
    ] {
        if !caps.lists(kind) || (kind == ObjectKind::Sequence && product == "MySQL") {
            continue;
        }
        let name = fixture_name(Group::Metadata, kind.as_str());
        let qualified = target
            .driver
            .dialect()
            .qualified(&target.schema, &name)
            .map_err(|error| Failure::Setup(error.to_string()))?;
        let (create, drop) = statements(target.driver.dialect(), kind, &qualified)?;
        fixtures.push(Fixture {
            kind,
            name,
            create,
            drop,
        });
    }
    Ok(fixtures)
}

fn statements(dialect: Dialect, kind: ObjectKind, name: &str) -> Result<(String, String), Failure> {
    let (create, keyword) = match (dialect, kind) {
        (Dialect::Postgres, ObjectKind::MaterializedView) => (
            format!("CREATE MATERIALIZED VIEW {name} AS SELECT 1 AS id"),
            "MATERIALIZED VIEW",
        ),
        (Dialect::Postgres, ObjectKind::Function) => (
            format!(
                "CREATE FUNCTION {name}(value integer) RETURNS integer LANGUAGE sql AS 'SELECT value'"
            ),
            "FUNCTION",
        ),
        (Dialect::Postgres, ObjectKind::Procedure) => (
            format!("CREATE PROCEDURE {name}() LANGUAGE sql AS 'SELECT 1'"),
            "PROCEDURE",
        ),
        (Dialect::Postgres, ObjectKind::Type) => {
            (format!("CREATE TYPE {name} AS ENUM ('a', 'b')"), "TYPE")
        }
        (Dialect::SqlServer, ObjectKind::Function) => (
            format!("CREATE FUNCTION {name}(@value int) RETURNS int AS BEGIN RETURN @value END"),
            "FUNCTION",
        ),
        (Dialect::SqlServer, ObjectKind::Procedure) => {
            (format!("CREATE PROCEDURE {name} AS SELECT 1"), "PROCEDURE")
        }
        (Dialect::SqlServer, ObjectKind::Type) => (format!("CREATE TYPE {name} FROM int"), "TYPE"),
        (Dialect::MySql, ObjectKind::Function) => (
            format!(
                "CREATE FUNCTION {name}(value integer) RETURNS integer DETERMINISTIC RETURN value"
            ),
            "FUNCTION",
        ),
        (Dialect::MySql, ObjectKind::Procedure) => {
            (format!("CREATE PROCEDURE {name}() SELECT 1"), "PROCEDURE")
        }
        (Dialect::Duckdb, ObjectKind::Function) => {
            (format!("CREATE MACRO {name}(value) AS value + 1"), "MACRO")
        }
        (
            Dialect::Postgres | Dialect::SqlServer | Dialect::MySql | Dialect::Duckdb,
            ObjectKind::Sequence,
        ) => (format!("CREATE SEQUENCE {name} START WITH 17"), "SEQUENCE"),
        _ => {
            return Err(Failure::Contract(format!(
                "Missing Metadata fixture for {dialect:?} {kind:?}"
            )));
        }
    };
    Ok((create, format!("DROP {keyword} {name}")))
}

async fn check(
    target: &Target,
    connection: &mut dyn Connection,
    fixture: &Fixture,
) -> Result<(), Failure> {
    let facet = catalog(connection)?;
    let found = facet.list_objects(&target.schema, &[fixture.kind]).await?;
    ensure(
        found
            .iter()
            .any(|object| object.name == fixture.name && object.kind == fixture.kind),
        || format!("{} is absent from its kind listing", fixture.name),
    )?;
    let indexed = facet.search_index().await?;
    ensure(
        indexed.iter().any(|object| {
            object.schema == target.schema
                && object.name == fixture.name
                && object.kind == fixture.kind
        }),
        || format!("{} is absent from the name index", fixture.name),
    )?;
    {
        let detail = facet
            .object_detail(&ObjectIdentity {
                schema: target.schema.clone(),
                name: fixture.name.clone(),
                kind: fixture.kind,
                arguments: None,
            })
            .await?;
        ensure(
            !detail.attributes.is_empty() || !detail.sections.is_empty(),
            || format!("{:?} has no inspection metadata", fixture.kind),
        )?;
    }
    Ok(())
}

async fn optional_facets(target: &Target, connection: &mut dyn Connection) -> Result<(), Failure> {
    let caps = target.driver.capabilities();
    let facet = catalog(connection)?;
    let roles = facet.list_roles().await;
    if caps.supports(Feature::Roles) {
        ensure(!roles?.is_empty(), || {
            "Role catalog does not list the test account".to_owned()
        })?;
    } else {
        ensure(matches!(roles, Err(DriverError::Unsupported(_))), || {
            "Unsupported role catalog did not return Unsupported".to_owned()
        })?;
    }
    let extensions = facet.list_extensions().await;
    if caps.supports(Feature::Extensions) {
        extensions?;
    } else {
        ensure(
            matches!(extensions, Err(DriverError::Unsupported(_))),
            || "Unsupported extensions did not return Unsupported".to_owned(),
        )?;
    }
    Ok(())
}
