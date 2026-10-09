// SPDX-License-Identifier: Apache-2.0
use super::*;
use crate::schema::{ColumnDetail, ConstraintDetail};

fn column(table: &str, name: &str, primary_key: bool) -> ColumnRow {
    ColumnRow {
        table: table.into(),
        column: Some(DiagramColumn {
            name: name.into(),
            data_type: "integer".into(),
            nullable: !primary_key,
            primary_key,
        }),
    }
}

fn key(table: &str, name: &str, (column, target): (&str, &str)) -> ForeignKeyRow {
    let (target_table, target_column) = target.split_once('.').unwrap_or((target, ""));
    ForeignKeyRow {
        table: table.into(),
        name: name.into(),
        column: column.into(),
        target_schema: "public".into(),
        target_table: target_table.into(),
        target_column: target_column.into(),
    }
}

#[test]
fn rows_group_into_tables_and_composite_keys() {
    let diagram = assemble(
        "public",
        vec![
            column("lines", "order_id", true),
            column("lines", "no", true),
            ColumnRow {
                table: "empty".into(),
                column: None,
            },
            column("orders", "id", true),
            column("orders", "region", true),
        ],
        vec![
            key("lines", "lines_order", ("order_id", "orders.id")),
            key("lines", "lines_order", ("no", "orders.region")),
        ],
        10,
    );
    let names: Vec<_> = diagram.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["lines", "empty", "orders"]);
    assert!(diagram.tables[1].columns.is_empty());
    assert!(!diagram.truncated);
    assert_eq!(diagram.foreign_keys.len(), 1);
    let fk = &diagram.foreign_keys[0];
    assert_eq!(fk.columns, ["order_id", "no"]);
    assert_eq!(fk.target.columns, ["id", "region"]);
}

#[test]
fn truncation_drops_keys_to_missing_tables() {
    let diagram = assemble(
        "public",
        vec![
            column("a", "id", true),
            column("b", "a_id", false),
            column("c", "b_id", false),
        ],
        vec![
            key("b", "b_a", ("a_id", "a.id")),
            key("c", "c_b", ("b_id", "b.id")),
        ],
        2,
    );
    assert!(diagram.truncated);
    assert_eq!(diagram.tables.len(), 2);
    let names: Vec<_> = diagram
        .foreign_keys
        .iter()
        .map(|k| k.name.as_str())
        .collect();
    assert_eq!(names, ["b_a"]);
}

#[test]
fn keys_to_other_schemas_are_hidden_and_case_is_resolved() {
    let mut other = key("b", "b_x", ("x_id", "a.id"));
    other.target_schema = "audit".into();
    let diagram = assemble(
        "public",
        vec![column("Artist", "id", true), column("b", "x_id", false)],
        vec![other, key("b", "b_artist", ("x_id", "artist.id"))],
        5,
    );
    assert_eq!(diagram.foreign_keys.len(), 1);
    assert_eq!(diagram.foreign_keys[0].target.table, "Artist");
}

fn constraint(kind: ConstraintKind, columns: &[&str], target: Option<&str>) -> ConstraintDetail {
    ConstraintDetail {
        name: format!("{kind:?}"),
        kind,
        definition: String::new(),
        columns: columns.iter().map(|&c| c.into()).collect(),
        references: target.map(|table| ForeignTarget {
            schema: "main".into(),
            table: table.into(),
            columns: vec!["id".into()],
        }),
    }
}

#[test]
fn table_detail_flattens_to_rows() {
    let detail = TableDetail {
        columns: ["id", "artist_id"]
            .iter()
            .map(|&name| ColumnDetail {
                name: name.into(),
                data_type: "INTEGER".into(),
                nullable: name != "id",
                ..ColumnDetail::default()
            })
            .collect(),
        constraints: vec![
            constraint(ConstraintKind::Primary, &["id"], None),
            constraint(ConstraintKind::Foreign, &["artist_id"], Some("artist")),
        ],
        ..TableDetail::default()
    };
    let (mut columns, mut keys) = (Vec::new(), Vec::new());
    detail_rows("album", &detail, (&mut columns, &mut keys));
    columns.push(column("artist", "id", true));
    let diagram = assemble("main", columns, keys, 5);
    let album = &diagram.tables[0];
    assert!(album.columns[0].primary_key && !album.columns[1].primary_key);
    assert_eq!(diagram.foreign_keys[0].columns, ["artist_id"]);
    assert_eq!(diagram.foreign_keys[0].target.columns, ["id"]);
}
