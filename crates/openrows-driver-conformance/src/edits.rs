// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Edits`]: row keys, exact snapshots, generated DML
//! committed atomically, conflict rollback and transactional DDL. This is
//! data-safety code (docs/25 §0), so every step is checked by reading the
//! table back.

use openrows_driver_api::{Connection, Editor, Feature, editor};
use openrows_sql::Dialect;
use openrows_sql::edit::{CellValue, Changes, Edit, EditKind, EditTable, Values, edit_sql};
use openrows_sql::select::{BoundSql, TableRef};

use crate::dialect::{codes, create_table};
use crate::sink::{query, run_all, sqlstate};
use crate::{Failure, Group, Target, ensure, fixture_name};

/// What the fixture table holds after the first commit.
const AFTER_COMMIT: [[Option<&str>; 3]; 2] = [
    [Some("1"), Some("one"), Some("b")],
    [Some("3"), Some("three"), None],
];

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let table = fixture_name(Group::Edits, "t");
    let relation = qualified(dialect, &target.schema, &table)?;
    let mut connection = target.connect().await?;
    run_all(
        connection.as_mut(),
        &[
            create_table(&relation),
            format!(
                "INSERT INTO {relation} (id, name, note) VALUES (1, 'one', 'a'), (2, 'two', NULL)"
            ),
        ],
    )
    .await?;
    let place = TableRef {
        schema: target.schema.clone(),
        name: table,
    };
    let outcome = edits(connection.as_mut(), dialect, &place).await;
    let ddl = if outcome.is_ok()
        && target
            .driver
            .capabilities()
            .supports(Feature::TransactionalDdl)
    {
        transactional_ddl(connection.as_mut(), target, &relation).await
    } else {
        Ok(())
    };
    query(connection.as_mut(), &format!("DROP TABLE {relation}")).await?;
    connection.close().await?;
    outcome.and(ddl)
}

fn qualified(dialect: Dialect, schema: &str, name: &str) -> Result<String, Failure> {
    dialect
        .qualified(schema, name)
        .map_err(|error| Failure::Setup(error.to_string()))
}

async fn edits(
    connection: &mut dyn Connection,
    dialect: Dialect,
    place: &TableRef,
) -> Result<(), Failure> {
    let target = (place.schema.as_str(), place.name.as_str());
    let editor = editor(connection)?;
    let info = editor.row_edit_info(target.0, target.1).await?;
    ensure(info.key == ["id"] && info.protected.is_empty(), || {
        format!("row edit info {info:?}")
    })?;
    let table = EditTable {
        table: place.clone(),
        columns: vec!["id".into(), "name".into(), "note".into()],
        key: info.key,
        protected: info.protected,
    };
    let one = snapshot(editor, target, "1").await?;
    let two = snapshot(editor, target, "2").await?;
    let expected: Values = [("id", "1"), ("name", "one"), ("note", "a")]
        .into_iter()
        .map(|(name, value)| (name.to_owned(), Some(value.to_owned())))
        .collect();
    ensure(one == expected, || format!("snapshot {one:?}"))?;
    ensure(two.get("note") == Some(&None), || {
        format!("NULL snapshot {two:?}")
    })?;
    let batch = [
        update(one.clone(), "b"),
        Edit {
            kind: EditKind::Insert,
            original: Values::new(),
            values: text_changes(&[("id", "3"), ("name", "three")]),
        },
        Edit {
            kind: EditKind::Delete,
            original: two,
            values: Changes::new(),
        },
    ];
    editor
        .commit_edits(&generate(dialect, &table, &batch)?)
        .await?;
    rows_are(connection, place, dialect).await?;
    conflict(connection, dialect, (&table, one)).await
}

/// A stale guard (the original note was `a`, it is now `b`) rolls back the
/// whole set, including an insert that would have succeeded.
async fn conflict(
    connection: &mut dyn Connection,
    dialect: Dialect,
    (table, stale): (&EditTable, Values),
) -> Result<(), Failure> {
    let batch = [
        Edit {
            kind: EditKind::Insert,
            original: Values::new(),
            values: text_changes(&[("id", "4"), ("name", "four")]),
        },
        update(stale, "c"),
    ];
    let statements = generate(dialect, table, &batch)?;
    let result = editor(connection)?.commit_edits(&statements).await;
    let code = sqlstate(result, "stale commit")?;
    ensure(code == "40001", || format!("stale commit SQLSTATE {code}"))?;
    rows_are(connection, &table.table, dialect).await
}

/// A DDL set whose second statement fails leaves no trace of the first.
async fn transactional_ddl(
    connection: &mut dyn Connection,
    target: &Target,
    relation: &str,
) -> Result<(), Failure> {
    let dialect = target.driver.dialect();
    let extra = qualified(dialect, &target.schema, &fixture_name(Group::Edits, "ddl"))?;
    let statements = [create_table(&extra), create_table(relation)];
    let result = editor(connection)?.commit_ddl(&statements).await;
    sqlstate(result, "failing DDL set")?;
    let probe = query(connection, &format!("SELECT * FROM {extra}")).await;
    let code = sqlstate(probe, "table from a rolled-back DDL set")?;
    let expected = codes(dialect).undefined_table;
    ensure(code == expected, || {
        format!("rolled-back table probe SQLSTATE {code}, expected {expected}")
    })
}

async fn snapshot(
    editor: &mut dyn Editor,
    (schema, table): (&str, &str),
    id: &str,
) -> Result<Values, Failure> {
    let keys = [("id".to_owned(), id.to_owned())];
    Ok(editor
        .edit_snapshot((schema, table), &keys)
        .await?
        .into_iter()
        .collect())
}

fn update(original: Values, note: &str) -> Edit {
    Edit {
        kind: EditKind::Update,
        original,
        values: text_changes(&[("note", note)]),
    }
}

fn text_changes(values: &[(&str, &str)]) -> Changes {
    values
        .iter()
        .map(|(name, value)| ((*name).to_owned(), CellValue::Text((*value).to_owned())))
        .collect()
}

fn generate(dialect: Dialect, table: &EditTable, batch: &[Edit]) -> Result<Vec<BoundSql>, Failure> {
    batch
        .iter()
        .map(|edit| edit_sql(dialect, table, edit))
        .collect::<Result<_, _>>()
        .map_err(|error| Failure::Contract(format!("edit SQL: {error}")))
}

/// The table holds exactly [`AFTER_COMMIT`].
async fn rows_are(
    connection: &mut dyn Connection,
    place: &TableRef,
    dialect: Dialect,
) -> Result<(), Failure> {
    let relation = qualified(dialect, &place.schema, &place.name)?;
    let columns = ["id", "name", "note"].map(|column| dialect.text_of(column));
    let sql = format!("SELECT {} FROM {relation} ORDER BY id", columns.join(", "));
    let (sink, _) = query(connection, &sql).await?;
    let found = [sink.column(0)?, sink.column(1)?, sink.column(2)?];
    let expected: [Vec<Option<String>>; 3] = std::array::from_fn(|column| {
        AFTER_COMMIT
            .iter()
            .map(|row| row[column].map(str::to_owned))
            .collect()
    });
    ensure(found == expected, || {
        format!("table holds {found:?}, expected {expected:?}")
    })
}
