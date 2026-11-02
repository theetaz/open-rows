// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Introspection`]: a fixture schema reads back through the
//! catalog exactly as `golden/<product>.txt` says.
//!
//! Each `golden/<product>.sql` holds the set-up, a `-- teardown` line and the
//! teardown, which also runs first so an aborted run cannot leave objects
//! behind. Volatile values (sizes, row estimates) are not rendered. After an
//! intended change, regenerate the text with `OPENROWS_UPDATE_GOLDEN=1` and
//! review its diff.

use std::fmt::Write as _;

use openrows_driver_api::catalog::{ObjectKind, ObjectSummary};
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::schema::TableDetail;
use openrows_driver_api::{Catalog, Connection, Feature, catalog};
use openrows_sql::{Dialect, split};

use crate::sink::query;
use crate::{Failure, Target, ensure};

/// One product's fixture and its expected catalog output.
struct Fixture {
    name: &'static str,
    sql: &'static str,
    expected: &'static str,
    /// The schema it creates; `None` when it fills the target's own.
    schema: Option<&'static str>,
}

/// The schema `PostgreSQL` and `CockroachDB` fixtures create for themselves.
const OWN_SCHEMA: &str = "orc_golden";

const POSTGRES: Fixture = Fixture {
    name: "postgres",
    sql: include_str!("../golden/postgres.sql"),
    expected: include_str!("../golden/postgres.txt"),
    schema: Some(OWN_SCHEMA),
};

const COCKROACH: Fixture = Fixture {
    name: "cockroachdb",
    sql: include_str!("../golden/cockroachdb.sql"),
    expected: include_str!("../golden/cockroachdb.txt"),
    schema: Some(OWN_SCHEMA),
};

const MYSQL: Fixture = Fixture {
    name: "mysql",
    sql: include_str!("../golden/mysql.sql"),
    expected: include_str!("../golden/mysql.txt"),
    schema: None,
};

const MARIADB: Fixture = Fixture {
    name: "mariadb",
    sql: include_str!("../golden/mysql.sql"),
    expected: include_str!("../golden/mariadb.txt"),
    schema: None,
};

const TIDB: Fixture = Fixture {
    name: "tidb",
    sql: include_str!("../golden/tidb.sql"),
    expected: include_str!("../golden/tidb.txt"),
    schema: None,
};

const SQLITE: Fixture = Fixture {
    name: "sqlite",
    sql: include_str!("../golden/sqlite.sql"),
    expected: include_str!("../golden/sqlite.txt"),
    schema: None,
};

fn fixture(connection: &dyn Connection) -> Result<&'static Fixture, Failure> {
    let server = connection.server();
    match (server.product.as_str(), server.flavour.as_deref()) {
        ("PostgreSQL", _) => Ok(&POSTGRES),
        ("CockroachDB", _) => Ok(&COCKROACH),
        ("MySQL", Some("TiDB")) => Ok(&TIDB),
        ("MySQL", None) => Ok(&MYSQL),
        ("MariaDB", _) => Ok(&MARIADB),
        ("SQLite", _) => Ok(&SQLITE),
        (product, flavour) => Err(Failure::Setup(format!(
            "no golden fixture for {product} {flavour:?}"
        ))),
    }
}

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let mut connection = target.connect().await?;
    let fixture = fixture(connection.as_ref())?;
    let (setup, teardown) = fixture
        .sql
        .split_once("-- teardown")
        .ok_or_else(|| Failure::Setup(format!("{} has no teardown", fixture.name)))?;
    script(connection.as_mut(), teardown, dialect).await?;
    script(connection.as_mut(), setup, dialect).await?;
    let schema = fixture.schema.unwrap_or(&target.schema);
    let rendered = {
        let capabilities = target.driver.capabilities();
        let routines = capabilities.supports(Feature::RoutineEditing);
        let catalog = catalog(connection.as_mut())?;
        render(catalog, schema, capabilities.object_kinds, routines).await
    };
    script(connection.as_mut(), teardown, dialect).await?;
    connection.close().await?;
    compare(fixture, &rendered?)
}

async fn script(
    connection: &mut dyn Connection,
    sql: &str,
    dialect: Dialect,
) -> Result<(), Failure> {
    for statement in split(sql, dialect) {
        let text = statement.text(sql).trim();
        if text.lines().all(|line| line.trim_start().starts_with("--")) {
            continue;
        }
        query(connection, text)
            .await
            .map_err(|error| Failure::Setup(format!("{text}: {error}")))?;
    }
    Ok(())
}

fn compare(fixture: &Fixture, rendered: &str) -> Result<(), Failure> {
    if std::env::var_os("OPENROWS_UPDATE_GOLDEN").is_some() {
        let path = format!("{}/golden/{}.txt", env!("CARGO_MANIFEST_DIR"), fixture.name);
        return std::fs::write(&path, rendered)
            .map_err(|error| Failure::Setup(format!("{path}: {error}")));
    }
    let first = fixture
        .expected
        .lines()
        .zip(rendered.lines())
        .position(|(expected, found)| expected != found);
    ensure(fixture.expected == rendered, || {
        let at = first.unwrap_or_else(|| {
            fixture
                .expected
                .lines()
                .count()
                .min(rendered.lines().count())
        });
        format!(
            "golden/{}.txt differs at line {}:\n  expected {:?}\n  found    {:?}\n\
             regenerate with OPENROWS_UPDATE_GOLDEN=1 if intended; output:\n{rendered}",
            fixture.name,
            at + 1,
            fixture.expected.lines().nth(at),
            rendered.lines().nth(at),
        )
    })
}

async fn render(
    catalog: &mut dyn Catalog,
    schema: &str,
    kinds: &[ObjectKind],
    routines: bool,
) -> Result<String, Failure> {
    let mut out = String::new();
    for object in catalog.list_objects(schema, kinds).await? {
        // A shared `MySQL` database or `SQLite` file also holds other tests' tables.
        if schema != OWN_SCHEMA && !object.name.starts_with("orc_golden") {
            continue;
        }
        object_line(&mut out, &object);
        match object.kind {
            ObjectKind::Table | ObjectKind::View => {
                let detail = catalog.table_detail(schema, &object.name).await?;
                detail_lines(&mut out, &detail);
            }
            ObjectKind::Function | ObjectKind::Procedure if routines => {
                routine_lines(&mut out, catalog, schema, &object).await?;
            }
            _ => {}
        }
    }
    Ok(out)
}

fn object_line(out: &mut String, object: &ObjectSummary) {
    let _ = writeln!(
        out,
        "{} {} arguments={:?} comment={:?}",
        object.kind.as_str(),
        object.name,
        object.arguments,
        object.comment
    );
}

async fn routine_lines(
    out: &mut String,
    catalog: &mut dyn Catalog,
    schema: &str,
    object: &ObjectSummary,
) -> Result<(), Failure> {
    let kind = if object.kind == ObjectKind::Procedure {
        RoutineKind::Procedure
    } else {
        RoutineKind::Function
    };
    let definition = catalog
        .routine_definition((schema, &object.name), kind, object.arguments.as_deref())
        .await?;
    let _ = writeln!(out, "  routine {definition:?}");
    Ok(())
}

fn detail_lines(out: &mut String, detail: &TableDetail) {
    if let Some(info) = &detail.info {
        let _ = writeln!(
            out,
            "  info kind={:?} comment={:?} partition={:?}",
            info.kind, info.comment, info.partition_key
        );
        let _ = writeln!(out, "  view {:?}", info.view_definition);
        let _ = writeln!(out, "  definition {:?}", info.definition);
    }
    for column in &detail.columns {
        let _ = writeln!(
            out,
            "  column {} type={:?} nullable={} default={:?} generated={:?} comment={:?}",
            column.name,
            column.data_type,
            column.nullable,
            column.default,
            column.generated,
            column.comment
        );
    }
    for constraint in &detail.constraints {
        let _ = writeln!(
            out,
            "  constraint {} kind={:?} columns={:?} references={:?} definition={:?}",
            constraint.name,
            constraint.kind,
            constraint.columns,
            constraint.references,
            constraint.definition
        );
    }
    for index in &detail.indexes {
        let _ = writeln!(
            out,
            "  index {} method={} unique={} primary={} constraint={} columns={:?} include={:?} \
             predicate={:?} comment={:?} definition={:?}",
            index.name,
            index.method,
            index.unique,
            index.primary,
            index.constraint,
            index.columns,
            index.include,
            index.predicate,
            index.comment,
            index.definition
        );
    }
    for trigger in &detail.triggers {
        let _ = writeln!(
            out,
            "  trigger {} timing={:?} events={:?} row={} function={:?} enabled={} definition={:?}",
            trigger.name,
            trigger.timing,
            trigger.events,
            trigger.row_level,
            trigger.function,
            trigger.enabled,
            trigger.definition
        );
    }
}
