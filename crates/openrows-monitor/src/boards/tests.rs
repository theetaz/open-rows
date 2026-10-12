// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;

use arrow_array::{Float64Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use openrows_sql::Dialect;

use super::*;

fn param(name: &str, kind: ParamKind) -> BoardParam {
    BoardParam {
        name: name.into(),
        kind,
        default: None,
        options: if kind == ParamKind::Enum {
            vec!["eu".into(), "us".into()]
        } else {
            Vec::new()
        },
    }
}

fn widget(sql: &str) -> Widget {
    Widget {
        id: "w1".into(),
        title: "Orders".into(),
        kind: WidgetKind::Chart,
        sql: sql.into(),
        chart: Some(ChartMapping {
            mark: Mark::Line,
            x: "day".into(),
            y: vec!["total".into()],
        }),
        limits: WidgetLimits::default(),
    }
}

fn board(sql: &str) -> Board {
    Board {
        version: BOARD_VERSION,
        name: "Sales".into(),
        params: vec![
            param("region", ParamKind::Enum),
            param("since", ParamKind::Date),
            param("min", ParamKind::Number),
            param("note", ParamKind::Text),
        ],
        widgets: vec![widget(sql)],
        refresh_secs: Some(60),
    }
}

fn value(name: &str, text: &str) -> ParamValue {
    ParamValue {
        name: name.into(),
        value: Some(text.into()),
    }
}

const SQL: &str = "SELECT day, total FROM sales WHERE region = :region AND day >= :since \
                   AND total > :min AND note <> :note AND region <> :region";

#[test]
fn typed_values_are_bound_never_spliced() -> Result<(), BoardError> {
    let board = board(SQL);
    let hostile = "x'; DROP TABLE sales; --";
    let values = [
        value("region", "eu"),
        value("since", "2026-02-28"),
        value("min", "10.5"),
        value("note", hostile),
    ];
    let bound = bind_widget(&board, &board.widgets[0], Dialect::Postgres, &values)?;
    assert!(bound.sql.contains("region = $1") && bound.sql.contains("region <> $5"));
    assert!(!bound.sql.contains("DROP") && !bound.sql.contains(":region"));
    let expected = ["eu", "2026-02-28", "10.5", hostile, "eu"].map(|v| Some(v.to_owned()));
    assert_eq!(bound.params, expected);
    let mysql = bind_widget(&board, &board.widgets[0], Dialect::MySql, &values)?;
    assert_eq!(mysql.sql.matches('?').count(), 5);
    Ok(())
}

#[test]
fn values_must_match_their_declared_type() {
    let board = board(SQL);
    let base = [
        value("region", "eu"),
        value("since", "2026-02-28"),
        value("min", "1"),
        value("note", "n"),
    ];
    for (index, bad) in [
        value("region", "apac"),
        value("since", "2026-02-30"),
        value("min", "1; DROP"),
        value("min", "NaN"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut values = base.clone();
        values[index] = bad;
        let bound = bind_widget(&board, &board.widgets[0], Dialect::Postgres, &values);
        assert!(matches!(bound, Err(BoardError::Param(_))), "{values:?}");
    }
    let missing = bind_widget(&board, &board.widgets[0], Dialect::Postgres, &base[..3]);
    assert!(matches!(missing, Err(BoardError::Param(_))));
}

#[test]
fn defaults_fill_empty_values_and_undeclared_names_are_refused() -> Result<(), BoardError> {
    let mut board = board("SELECT :min AS total");
    board.params[2].default = Some("5".into());
    let bound = bind_widget(
        &board,
        &board.widgets[0],
        Dialect::Sqlite,
        &[value("min", "")],
    )?;
    assert_eq!(bound.params, vec![Some("5".to_owned())]);
    board.widgets[0].sql = "SELECT :other".into();
    let undeclared = bind_widget(&board, &board.widgets[0], Dialect::Sqlite, &[]);
    assert!(matches!(undeclared, Err(BoardError::Param(_))));
    Ok(())
}

#[test]
fn only_a_single_read_statement_is_admitted() {
    assert!(admit_sql("SELECT 1", Dialect::Postgres).is_ok());
    for sql in [
        "DELETE FROM t",
        "INSERT INTO t VALUES (1)",
        "DROP TABLE t",
        "SELECT 1; DELETE FROM t",
        "BEGIN",
        "UPDATE t SET a = 1",
        "WITH d AS (DELETE FROM t RETURNING *) SELECT * FROM d",
    ] {
        assert!(admit_sql(sql, Dialect::Postgres).is_err(), "{sql}");
    }
}

#[test]
fn import_round_trips_and_rejects_bad_documents() -> Result<(), BoardError> {
    let board = board(SQL);
    let text = export_board(&board)?;
    assert_eq!(parse_board(&text)?, board);
    let other = text.replacen("\"version\": 1", "\"version\": 2", 1);
    assert_eq!(parse_board(&other), Err(BoardError::Version(2)));
    let oversize = format!("{text}{}", " ".repeat(MAX_BOARD_BYTES));
    assert_eq!(
        parse_board(&oversize),
        Err(BoardError::TooLarge(MAX_BOARD_BYTES))
    );
    for bad in [
        "{",
        "[]",
        "{\"version\": 1}",
        &text.replacen("\"name\"", "\"reviewed\": true, \"name\"", 1),
        &text.replacen("\"kind\": \"chart\"", "\"kind\": \"script\"", 1),
    ] {
        assert!(
            matches!(parse_board(bad), Err(BoardError::Malformed(_))),
            "{bad}"
        );
    }
    let mut invalid = board.clone();
    invalid.widgets[0].limits.max_rows = 1_000_000;
    assert!(matches!(
        export_board(&invalid),
        Err(BoardError::Invalid(_))
    ));
    invalid = board;
    invalid.refresh_secs = Some(1);
    assert!(matches!(validate(&invalid), Err(BoardError::Invalid(_))));
    Ok(())
}

fn batch(rows: i64) -> Result<RecordBatch, arrow_schema::ArrowError> {
    let schema = Schema::new(vec![
        Field::new("day", DataType::Int64, false),
        Field::new("total", DataType::Float64, true),
        Field::new("label", DataType::Utf8, false),
    ]);
    let days: Vec<i64> = (0..rows).collect();
    #[expect(clippy::cast_precision_loss, reason = "small fixture values")]
    let totals: Vec<Option<f64>> = days
        .iter()
        .map(|d| (d % 2 == 0).then_some(*d as f64))
        .collect();
    let labels: Vec<String> = days.iter().map(|d| format!("row {d}")).collect();
    RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(Int64Array::from(days)),
            Arc::new(Float64Array::from(totals)),
            Arc::new(StringArray::from(labels)),
        ],
    )
}

#[test]
fn collector_types_columns_and_enforces_row_and_byte_limits() -> Result<(), arrow_schema::ArrowError>
{
    let first = batch(10)?;
    let limits = WidgetLimits {
        max_rows: 4,
        ..WidgetLimits::default()
    };
    let mut collector = Collector::new(limits);
    collector.begin(&first.schema());
    assert!(!collector.push(&first));
    let result = collector.finish();
    let kinds: Vec<ColumnKind> = result.columns.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds,
        [ColumnKind::Number, ColumnKind::Number, ColumnKind::Text]
    );
    assert_eq!(result.rows.len(), 4);
    assert_eq!(result.truncated, Some(Truncation::Rows));
    assert_eq!(result.rows[1][1], None);
    assert_eq!(result.rows[2][1].as_deref(), Some("2.0"));
    let limits = WidgetLimits {
        max_bytes: 1_024,
        ..WidgetLimits::default()
    };
    let mut collector = Collector::new(limits);
    collector.begin(&first.schema());
    assert!(!collector.push(&batch(500)?));
    let result = collector.finish();
    assert_eq!(result.truncated, Some(Truncation::Bytes));
    assert!(result.rows.len() < 500);
    Ok(())
}
