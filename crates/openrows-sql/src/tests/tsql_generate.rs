// SPDX-License-Identifier: Apache-2.0
//! Table-browse, row-edit and DDL helpers for SQL Server: bracket quoting,
//! `@Pn` placeholders, `OFFSET … FETCH` paging and exact guards
//! (data-safety critical).

use crate::ddl::{string_literal, valid_type};
use crate::edit::{CellValue, Changes, Edit, EditKind, EditTable, Values, edit_sql};
use crate::select::{
    Combinator, Filter, FilterOp, Page, SelectError, Sort, SortDirection, TableQuery, TableRef,
    count_sql, page_sql,
};
use crate::{Dialect, StatementKind, classify};

const MS: Dialect = Dialect::SqlServer;

fn known() -> Vec<String> {
    ["id", "name", "we]ird"].map(str::to_owned).to_vec()
}

fn query(filters: Vec<Filter>, sort: Option<Sort>) -> TableQuery {
    TableQuery {
        table: TableRef {
            schema: "dbo".into(),
            name: "or]ders".into(),
        },
        columns: Vec::new(),
        filters,
        groups: Vec::new(),
        combinator: Combinator::And,
        sort,
        search: None,
        row_id: None,
    }
}

fn filter(column: &str, op: FilterOp, value: &str) -> Filter {
    Filter {
        column: column.into(),
        op,
        value: value.into(),
        second: String::new(),
    }
}

#[test]
fn pages_fetch_after_an_order() -> Result<(), SelectError> {
    let filters = vec![
        filter("id", FilterOp::Greater, "5"),
        filter("name", FilterOp::Contains, "50%_[x]"),
    ];
    let sort = Some(Sort {
        column: "we]ird".into(),
        direction: SortDirection::Descending,
    });
    let page = Page {
        limit: 100,
        offset: 200,
    };
    let bound = page_sql(MS, &query(filters, sort), &known(), page)?;
    assert_eq!(
        bound.sql,
        "SELECT * FROM [dbo].[or]]ders] WHERE ([id] > @P1) AND \
         (CONVERT(NVARCHAR(MAX), [name]) LIKE @P2 ESCAPE '\\') \
         ORDER BY [we]]ird] DESC OFFSET 200 ROWS FETCH NEXT 100 ROWS ONLY"
    );
    assert_eq!(
        bound.params,
        [Some("5".to_owned()), Some("%50\\%\\_\\[x]%".to_owned())]
    );
    assert_eq!(classify(&bound.sql, MS).kind, StatementKind::Read);
    let unsorted = page_sql(MS, &query(Vec::new(), None), &known(), page)?;
    assert!(
        unsorted
            .sql
            .ends_with("ORDER BY (SELECT NULL) OFFSET 200 ROWS FETCH NEXT 100 ROWS ONLY")
    );
    let empty = Page {
        limit: 0,
        offset: 0,
    };
    let none = page_sql(MS, &query(Vec::new(), None), &known(), empty)?;
    assert_eq!(none.sql, "SELECT TOP (0) * FROM [dbo].[or]]ders]");
    let count = count_sql(MS, &query(Vec::new(), None), &known())?;
    assert_eq!(count.sql, "SELECT COUNT_BIG(*) FROM [dbo].[or]]ders]");
    let regex = page_sql(
        MS,
        &query(vec![filter("name", FilterOp::Regex, "^a")], None),
        &known(),
        page,
    );
    assert!(matches!(regex, Err(SelectError::Unsupported(_))));
    Ok(())
}

fn table() -> EditTable {
    EditTable {
        table: TableRef {
            schema: "dbo".into(),
            name: "t".into(),
        },
        columns: vec!["id".into(), "note".into()],
        key: vec!["id".into()],
        protected: Vec::new(),
    }
}

fn values(pairs: &[(&str, Option<&str>)]) -> Values {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.map(str::to_owned)))
        .collect()
}

#[test]
fn edits_guard_every_column_exactly() -> Result<(), SelectError> {
    let original = values(&[("id", Some("1")), ("note", None)]);
    let changes: Changes = [("note".to_owned(), CellValue::Text("x".into()))].into();
    let update = edit_sql(
        MS,
        &table(),
        &Edit {
            kind: EditKind::Update,
            original: original.clone(),
            values: changes,
        },
    )?;
    assert_eq!(
        update.sql,
        "UPDATE [dbo].[t] SET [note] = @P1 WHERE \
         (CONVERT(VARBINARY(MAX), CONVERT(NVARCHAR(MAX), [id])) = CONVERT(VARBINARY(MAX), @P2) \
         OR ([id] IS NULL AND @P2 IS NULL)) AND \
         (CONVERT(VARBINARY(MAX), CONVERT(NVARCHAR(MAX), [note])) = CONVERT(VARBINARY(MAX), @P3) \
         OR ([note] IS NULL AND @P3 IS NULL))"
    );
    assert_eq!(
        update.params,
        [Some("x".to_owned()), Some("1".to_owned()), None]
    );
    let insert = edit_sql(
        MS,
        &table(),
        &Edit {
            kind: EditKind::Insert,
            original: Values::new(),
            values: [("note".to_owned(), CellValue::Default)].into(),
        },
    )?;
    assert_eq!(insert.sql, "INSERT INTO [dbo].[t] DEFAULT VALUES");
    let reset = edit_sql(
        MS,
        &table(),
        &Edit {
            kind: EditKind::Update,
            original,
            values: [("note".to_owned(), CellValue::Default)].into(),
        },
    )?;
    assert!(
        reset
            .sql
            .starts_with("UPDATE [dbo].[t] SET [note] = DEFAULT WHERE ")
    );
    Ok(())
}

#[test]
fn ddl_helpers_speak_t_sql() {
    assert_eq!(
        string_literal(MS, "it's \\ ok").as_deref(),
        Some("N'it''s \\ ok'")
    );
    for good in [
        "nvarchar(max)",
        "VARBINARY(MAX)",
        "decimal(10, 2)",
        "[dbo].[money_t]",
    ] {
        assert!(valid_type(MS, good), "{good}");
    }
    for bad in ["int; DROP TABLE t", "varchar(max) --", "nvarchar(min)"] {
        assert!(!valid_type(MS, bad), "{bad}");
    }
    assert!(!valid_type(Dialect::Postgres, "varchar(max)"));
}
