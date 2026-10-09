// SPDX-License-Identifier: Apache-2.0
use super::*;

#[test]
fn type_names_follow_a_conservative_grammar_per_dialect() {
    for (dialect, text) in [
        (Dialect::Postgres, "numeric(10,-2)"),
        (Dialect::Postgres, "int4[3][]"),
        (Dialect::Postgres, "timestamp(3) with time zone"),
        (Dialect::Postgres, "pg_catalog.\"char\""),
        (Dialect::MySql, "decimal(10, 2) unsigned zerofill"),
        (Dialect::MySql, "enum('a', 'b''c')"),
        (Dialect::MySql, "varchar(40) CHARACTER SET utf8mb4"),
        (Dialect::Sqlite, "VARCHAR(40)"),
        (Dialect::Sqlite, "[my type]"),
    ] {
        assert!(valid_type(dialect, text), "{dialect:?} {text}");
    }
    for (dialect, text) in [
        (Dialect::Postgres, "int; DROP TABLE t"),
        (Dialect::Postgres, "int -- x"),
        (Dialect::Postgres, "numeric(a)"),
        (Dialect::Postgres, "text[x]"),
        (Dialect::Postgres, "enum('a')"),
        (Dialect::MySql, "int DEFAULT 'x'"),
        (Dialect::MySql, "int /* x */"),
        (Dialect::MySql, "int,int"),
        (Dialect::Sqlite, "int)"),
        (Dialect::Sqlite, ""),
    ] {
        assert!(!valid_type(dialect, text), "{dialect:?} {text:?}");
    }
}

#[test]
fn literals_escape_per_dialect() {
    assert_eq!(
        string_literal(Dialect::MySql, r"it's \n").as_deref(),
        Some(r"'it''s \\n'")
    );
    assert_eq!(
        string_literal(Dialect::Sqlite, r"it's \n").as_deref(),
        Some(r"'it''s \n'")
    );
    assert_eq!(string_literal(Dialect::Sqlite, "a\0"), None);
}

#[test]
fn created_index_and_trigger_names_gain_their_schema() {
    let qualify = |sql| qualify_created_name(sql, Dialect::Sqlite, "\"aux\"");
    assert_eq!(
        qualify("CREATE UNIQUE INDEX IF NOT EXISTS ix ON t (a)").as_deref(),
        Some("CREATE UNIQUE INDEX IF NOT EXISTS \"aux\".ix ON t (a)")
    );
    assert_eq!(
        qualify("CREATE TRIGGER \"tr\" AFTER INSERT ON t BEGIN SELECT 1; END").as_deref(),
        Some("CREATE TRIGGER \"aux\".\"tr\" AFTER INSERT ON t BEGIN SELECT 1; END")
    );
    assert_eq!(
        qualify("CREATE INDEX main.ix ON t (a)").as_deref(),
        Some("CREATE INDEX main.ix ON t (a)")
    );
    assert_eq!(qualify("SELECT 1"), None);
}

const SQLITE_TABLE: &str = "CREATE TABLE \"orders\" (
    id INTEGER PRIMARY KEY AUTOINCREMENT, -- the key
    \"customer id\" INTEGER NOT NULL ON CONFLICT ABORT REFERENCES customers(id) ON DELETE CASCADE,
    note TEXT DEFAULT 'n/a' COLLATE NOCASE,
    total NUMERIC CONSTRAINT positive CHECK (total > 0),
    CONSTRAINT one UNIQUE (\"customer id\", note)
) WITHOUT ROWID, STRICT";

#[test]
fn create_table_splits_into_columns_and_constraints() -> Result<(), &'static str> {
    let table = TableDefinition::parse(SQLITE_TABLE, Dialect::Sqlite).ok_or("parse")?;
    assert_eq!(table.head, "CREATE TABLE \"orders\" (");
    assert_eq!(table.tail, ") WITHOUT ROWID, STRICT");
    let columns: Vec<_> = table
        .elements
        .iter()
        .map(|element| element.column.as_deref())
        .collect();
    assert_eq!(
        columns,
        [
            Some("id"),
            Some("customer id"),
            Some("note"),
            Some("total"),
            None
        ]
    );
    assert_eq!(table.render(), SQLITE_TABLE);
    assert!(TableDefinition::parse("CREATE TABLE t AS SELECT 1", Dialect::Sqlite).is_none());
    assert!(TableDefinition::parse("CREATE VIEW v AS SELECT (1)", Dialect::Sqlite).is_none());
    Ok(())
}

fn column(name: &str) -> Result<ColumnDefinition, &'static str> {
    let table = TableDefinition::parse(SQLITE_TABLE, Dialect::Sqlite).ok_or("parse")?;
    let element = table.column(name).ok_or("column")?;
    ColumnDefinition::parse(&element.text, Dialect::Sqlite).ok_or("column definition")
}

#[test]
fn column_edits_keep_every_other_clause() -> Result<(), &'static str> {
    let mut customer = column("customer id")?;
    assert_eq!(
        customer.render(),
        "\"customer id\" INTEGER NOT NULL ON CONFLICT ABORT REFERENCES customers(id) ON DELETE CASCADE"
    );
    customer.set_nullable(true);
    customer.set_type("BIGINT");
    assert_eq!(
        customer.render(),
        "\"customer id\" BIGINT REFERENCES customers(id) ON DELETE CASCADE"
    );
    customer.set_nullable(false);
    customer.set_default(Some("0"));
    assert_eq!(
        customer.render(),
        "\"customer id\" BIGINT NOT NULL DEFAULT 0 REFERENCES customers(id) ON DELETE CASCADE"
    );
    let mut id = column("id")?;
    id.set_default(Some("(abs(random()))"));
    assert_eq!(
        id.render(),
        "id INTEGER DEFAULT (abs(random())) PRIMARY KEY AUTOINCREMENT"
    );
    Ok(())
}

#[test]
fn type_changes_drop_the_collation_and_defaults_replace_in_place() -> Result<(), &'static str> {
    let mut note = column("note")?;
    note.set_default(Some("'none'"));
    assert_eq!(note.render(), "note TEXT DEFAULT 'none' COLLATE NOCASE");
    note.set_type("VARCHAR(10)");
    note.set_default(None);
    assert_eq!(note.render(), "note VARCHAR(10)");
    let mut total = column("total")?;
    total.set_nullable(false);
    assert_eq!(
        total.render(),
        "total NUMERIC NOT NULL CONSTRAINT positive CHECK (total > 0)"
    );
    Ok(())
}

#[test]
fn mysql_columns_keep_charset_auto_increment_and_on_update() -> Result<(), &'static str> {
    let text = "`updated` timestamp NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'last change'";
    let mut updated = ColumnDefinition::parse(text, Dialect::MySql).ok_or("parse")?;
    assert_eq!(updated.render(), text);
    updated.set_nullable(false);
    updated.set_comment(Some("'changed'"));
    assert_eq!(
        updated.render(),
        "`updated` timestamp NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP COMMENT 'changed'"
    );
    let text = "`name` varchar(40) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL DEFAULT ''";
    let mut name = ColumnDefinition::parse(text, Dialect::MySql).ok_or("parse")?;
    name.set_name("`full_name`");
    name.set_default(None);
    assert_eq!(
        name.render(),
        "`full_name` varchar(40) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL"
    );
    let generated = "`total` int GENERATED ALWAYS AS ((`a` + 1)) STORED";
    let mut total = ColumnDefinition::parse(generated, Dialect::MySql).ok_or("parse")?;
    total.set_nullable(false);
    assert_eq!(
        total.render(),
        "`total` int GENERATED ALWAYS AS ((`a` + 1)) STORED NOT NULL"
    );
    let mut fresh = ColumnDefinition::new("`n`", "int");
    fresh.set_nullable(false);
    fresh.set_default(Some("0"));
    assert_eq!(fresh.render(), "`n` int NOT NULL DEFAULT 0");
    Ok(())
}

#[test]
fn collations_auto_increment_and_references_are_replaced() -> Result<(), &'static str> {
    let mut column = ColumnDefinition::parse(
        "`name` varchar(40) CHARACTER SET latin1 COLLATE latin1_bin NOT NULL",
        Dialect::MySql,
    )
    .ok_or("parse")?;
    column.set_collation(Some("utf8mb4"), Some("utf8mb4_bin"));
    assert_eq!(
        column.render(),
        "`name` varchar(40) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL"
    );
    column.set_collation(None, None);
    assert_eq!(column.render(), "`name` varchar(40) NOT NULL");
    let mut id =
        ColumnDefinition::parse("`id` int NOT NULL DEFAULT '0' COMMENT 'k'", Dialect::MySql)
            .ok_or("parse")?;
    id.set_auto_increment(true);
    assert_eq!(
        id.render(),
        "`id` int NOT NULL DEFAULT '0' AUTO_INCREMENT COMMENT 'k'"
    );
    id.set_auto_increment(false);
    assert_eq!(id.render(), "`id` int NOT NULL DEFAULT '0' COMMENT 'k'");
    let mut fk = ColumnDefinition::parse(
        "owner INTEGER NOT NULL REFERENCES owners(id) ON DELETE CASCADE",
        Dialect::Sqlite,
    )
    .ok_or("parse")?;
    assert!(fk.remove_references());
    assert_eq!(fk.render(), "owner INTEGER NOT NULL");
    assert!(!fk.remove_references());
    Ok(())
}
