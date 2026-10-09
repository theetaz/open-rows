// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Metadata`]: databases, schemas, objects, the search
//! index and column listings of a fixture table and view.

use openrows_driver_api::catalog::{ObjectKind, ObjectSummary};
use openrows_driver_api::{Catalog, Connection, catalog};

use crate::dialect::create_table;
use crate::sink::{query, run_all};
use crate::{Failure, Group, Target, ensure, fixture_name};

/// Fixture names: `(schema, table, view)`.
type Names<'a> = (&'a str, &'a str, &'a str);

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let (table, view) = (
        fixture_name(Group::Metadata, "t"),
        fixture_name(Group::Metadata, "v"),
    );
    let qualify = |name: &str| {
        dialect
            .qualified(&target.schema, name)
            .map_err(|error| Failure::Setup(error.to_string()))
    };
    let (relation, view_relation) = (qualify(&table)?, qualify(&view)?);
    let mut connection = target.connect().await?;
    run_all(
        connection.as_mut(),
        &[
            create_table(&relation),
            format!("CREATE VIEW {view_relation} AS SELECT id, name FROM {relation}"),
        ],
    )
    .await?;
    let outcome = async {
        check(connection.as_mut(), (&target.schema, &table, &view)).await?;
        crate::metadata_objects::run(target, connection.as_mut()).await
    }
    .await;
    for drop in [
        format!("DROP VIEW {view_relation}"),
        format!("DROP TABLE {relation}"),
    ] {
        query(connection.as_mut(), &drop).await?;
    }
    connection.close().await?;
    outcome
}

async fn check(connection: &mut dyn Connection, names: Names<'_>) -> Result<(), Failure> {
    let database = connection.server().database.clone();
    let catalog = catalog(connection)?;
    let databases = catalog.list_databases().await?;
    ensure(databases.iter().any(|item| item.name == database), || {
        format!("databases do not list the current one, {database}")
    })?;
    let (schema, table, view) = names;
    let schemas = catalog.list_schemas().await?;
    ensure(schemas.iter().any(|item| item.name == schema), || {
        format!("schemas do not list {schema}")
    })?;
    let kinds = [ObjectKind::Table, ObjectKind::View];
    let objects = catalog.list_objects(schema, &kinds).await?;
    listed(&objects, names, "list_objects")?;
    let sorted = objects.windows(2).all(|pair| match pair {
        [a, b] => a.name <= b.name,
        _ => true,
    });
    ensure(sorted, || "objects are not ordered by name".to_owned())?;
    let index = catalog.search_index().await?;
    listed(&index, names, "search_index")?;
    columns(catalog, schema, table).await?;
    let missing = catalog
        .table_columns(schema, &format!("{view}_none"))
        .await?;
    ensure(missing.is_empty(), || {
        "a missing table has columns".to_owned()
    })
}

/// The table and view appear once each, with their kinds and schema.
fn listed(
    objects: &[ObjectSummary],
    (schema, table, view): Names<'_>,
    what: &str,
) -> Result<(), Failure> {
    for (name, kind) in [(table, ObjectKind::Table), (view, ObjectKind::View)] {
        let found: Vec<_> = objects
            .iter()
            .filter(|object| object.name == name && object.schema == schema)
            .collect();
        ensure(
            matches!(found.as_slice(), [one] if one.kind == kind),
            || format!("{what}: {name} listed as {found:?}, expected one {kind:?}"),
        )?;
    }
    Ok(())
}

/// Columns in ordinal order with nullability and the primary key; the
/// structure and detail views agree.
async fn columns(catalog: &mut dyn Catalog, schema: &str, table: &str) -> Result<(), Failure> {
    let summary = catalog.table_columns(schema, table).await?;
    let shape: Vec<_> = summary
        .iter()
        .map(|column| (column.name.as_str(), column.nullable, column.primary_key))
        .collect();
    let expected = [
        ("id", false, true),
        ("name", false, false),
        ("note", true, false),
    ];
    ensure(shape == expected, || {
        format!("columns {shape:?}, expected {expected:?}")
    })?;
    ensure(
        summary.iter().all(|column| !column.data_type.is_empty()),
        || "a column has no type".to_owned(),
    )?;
    let structure = catalog.table_structure(schema, table).await?;
    ensure(structure.columns == summary, || {
        "table_structure columns differ from table_columns".to_owned()
    })?;
    let detail = catalog.table_detail(schema, table).await?;
    let names: Vec<_> = detail
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect();
    ensure(names == ["id", "name", "note"], || {
        format!("table_detail columns {names:?}")
    })
}
