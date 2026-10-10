// SPDX-License-Identifier: Apache-2.0
//! Bound values, native types, row caps and conditional transaction rollback.
use super::support::{self, Collect};
use openrows_driver_api::{ConnectTrace, DiscardSink, DriverError, Session};
use openrows_driver_libsql::{NamedArg, Value, connect};
use openrows_sql::select::BoundSql;

#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn positional_and_named_injection_strings_remain_values() -> Result<(), DriverError> {
    let mut session = support::session().await?;
    session
        .sequence("CREATE TABLE IF NOT EXISTS libsql_injection (value TEXT)")
        .await?;
    let value = "x'); DROP TABLE libsql_injection;--";
    session
        .execute_bound(
            "INSERT INTO libsql_injection VALUES (?1)",
            &[Some(value.into())],
            &mut DiscardSink,
        )
        .await?;
    let mut sink = Collect::default();
    session
        .execute_values(
            "SELECT :name",
            Vec::new(),
            vec![NamedArg {
                name: ":name".into(),
                value: Value::Text {
                    value: value.into(),
                },
            }],
            &mut sink,
        )
        .await?;
    assert_eq!(sink.value(0, 0).as_deref(), Some(value));
    let mut stored = Collect::default();
    session
        .execute("SELECT value FROM libsql_injection", &mut stored)
        .await?;
    assert_eq!(stored.value(0, 0).as_deref(), Some(value));
    session
        .execute("DROP TABLE libsql_injection", &mut DiscardSink)
        .await?;
    Box::new(session).close().await
}

#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn native_types_include_full_integer_precision_and_binary() -> Result<(), DriverError> {
    let mut session = support::session().await?;
    let values = vec![
        Value::Integer {
            value: i64::MAX.to_string(),
        },
        Value::Float { value: 1.5 },
        Value::Text {
            value: "සිංහල".into(),
        },
        Value::Blob {
            base64: "AP8=".into(),
        },
        Value::Null,
    ];
    let mut sink = Collect::default();
    session
        .execute_values("SELECT ?1, ?2, ?3, ?4, ?5", values, Vec::new(), &mut sink)
        .await?;
    assert_eq!(sink.value(0, 0), Some(i64::MAX.to_string()));
    assert_eq!(sink.value(0, 2).as_deref(), Some("සිංහල"));
    let batch = sink
        .batches
        .first()
        .ok_or_else(|| DriverError::Protocol("missing test batch".into()))?;
    let binary = batch
        .column(3)
        .as_any()
        .downcast_ref::<arrow_array::BinaryArray>()
        .ok_or_else(|| DriverError::Protocol("missing blob".into()))?;
    assert_eq!(binary.value(0), &[0, 255]);
    Box::new(session).close().await
}

#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn row_cap_has_visible_notice_and_response_byte_cap_fails_safely() -> Result<(), DriverError>
{
    let mut request = support::request()?;
    request
        .driver_options
        .insert("libsql_row_cap".into(), "201".into());
    let mut session = connect(&request, &mut ConnectTrace::default()).await?;
    let sql = "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<1000) SELECT x FROM n";
    let mut sink = Collect::default();
    let summary = session.execute(sql, &mut sink).await?;
    assert_eq!(summary.rows, 201);
    assert_eq!(
        sink.batches
            .iter()
            .map(arrow_array::RecordBatch::num_rows)
            .collect::<Vec<_>>(),
        [200, 1]
    );
    assert_eq!(sink.notices, ["result truncated at 201 rows; add LIMIT"]);
    Box::new(session).close().await?;
    request
        .driver_options
        .insert("libsql_response_bytes".into(), "4096".into());
    let mut session = connect(&request, &mut ConnectTrace::default()).await?;
    let result = session
        .execute("SELECT hex(zeroblob(10000))", &mut Collect::default())
        .await;
    assert!(
        matches!(result, Err(DriverError::Unsupported(message)) if message.contains("add LIMIT"))
    );
    Box::new(session).close().await
}

#[tokio::test]
#[ignore = "needs authenticated sqld; run just test-libsql"]
async fn conditional_batch_failure_rolls_back_all_writes() -> Result<(), DriverError> {
    let mut session = support::session().await?;
    session
        .sequence("CREATE TABLE libsql_batch (id INTEGER PRIMARY KEY)")
        .await?;
    let statements = [1, 1, 2].map(|id| BoundSql {
        sql: "INSERT INTO libsql_batch VALUES (?1)".into(),
        params: vec![Some(id.to_string())],
    });
    let result = session
        .atomic_batch(&statements, &mut Collect::default())
        .await;
    assert!(matches!(result, Err(DriverError::Server { code, .. }) if code == "23505"));
    assert!(!session.in_transaction());
    let mut sink = Collect::default();
    session
        .execute("SELECT count(*) FROM libsql_batch", &mut sink)
        .await?;
    assert_eq!(sink.value(0, 0).as_deref(), Some("0"));
    session
        .execute("DROP TABLE libsql_batch", &mut DiscardSink)
        .await?;
    Box::new(session).close().await
}
