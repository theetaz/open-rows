// SPDX-License-Identifier: Apache-2.0
//! Native `DuckDB` query, type and file contracts through the production adapter.
#[path = "support/duckdb.rs"]
mod fixture;
#[cfg(target_os = "macos")]
#[path = "support/duckdb_image.rs"]
mod image;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use fixture::*;
use openrows_driver_api::{ResultSink, SinkFlow};
#[derive(Default)]
struct Collect {
    batches: Vec<RecordBatch>,
    schemas: Vec<SchemaRef>,
    stop: bool,
}
impl ResultSink for Collect {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.schemas.push(schema);
    }
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.batches.push(batch);
        if self.stop {
            SinkFlow::Stop
        } else {
            SinkFlow::Continue
        }
    }
}

use openrows_driver_api::{Connection, DiscardSink, DriverError, FileSource, Session};

#[tokio::test]
async fn create_open_read_only_memory_binding_and_affected_rows()
-> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    image::prepare().await?;
    tokio::time::timeout(std::time::Duration::from_secs(10), file_contract())
        .await
        .map_err(|_| "file contract timed out")??;
    Ok(())
}
async fn file_contract() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir_in(".")?;
    let path = std::fs::canonicalize(root.path())?.join("data.duckdb");
    let source = FileSource::Disk {
        path,
        read_only: false,
        create: true,
    };
    let mut connection = open(source.clone()).await?;
    assert!(connection.catalog().is_some() && connection.editor().is_some());
    connection
        .execute(
            "CREATE TABLE items(id INTEGER, value VARCHAR)",
            &mut DiscardSink,
        )
        .await?;
    let summary = connection
        .execute_bound(
            "INSERT INTO items VALUES ($1, $2)",
            &[Some("7".into()), Some("héllo 'quote'".into())],
            &mut DiscardSink,
        )
        .await?;
    assert_eq!(summary.command_tag, "INSERT 1");
    let mut sink = Collect::default();
    connection
        .execute_bound("SELECT value, $1::VARCHAR FROM items", &[None], &mut sink)
        .await?;
    assert_eq!(
        sink.batches[0]
            .column(0)
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .ok_or("string")?
            .value(0),
        "héllo 'quote'"
    );
    assert!(sink.batches[0].column(1).is_null(0));
    Box::new(connection).close().await?;
    let FileSource::Disk { path, .. } = source else {
        return Err("disk".into());
    };
    let mut read = open(FileSource::Disk {
        path,
        read_only: true,
        create: false,
    })
    .await?;
    assert_eq!(
        read.execute("SELECT * FROM items", &mut DiscardSink)
            .await?
            .rows,
        1
    );
    assert!(matches!(
        read.execute("INSERT INTO items VALUES (8, 'x')", &mut DiscardSink)
            .await,
        Err(DriverError::Server { .. })
    ));
    Box::new(read).close().await?;
    let mut mem = open(memory("native-file-test")).await?;
    assert!(
        mem.server()
            .flavour
            .as_deref()
            .is_some_and(|v| v.contains("permanently loses"))
    );
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        mem.execute("SELECT 1; SELECT 2", &mut Collect::default()),
    )
    .await
    .map_err(|_| "multiple statement timeout")??;
    Box::new(mem).close().await?;
    Ok(())
}
#[tokio::test]
async fn locked_security_settings_and_attach_refusal_preserve_session()
-> Result<(), Box<dyn std::error::Error>> {
    let mut connection = open(memory("security-test")).await?;
    for sql in [
        "SET autoinstall_known_extensions=true",
        "SET autoload_known_extensions=true",
        "SET allow_persistent_secrets=true",
        "SET lock_configuration=false",
        "INSTALL httpfs",
    ] {
        assert!(
            matches!(
                connection.execute(sql, &mut DiscardSink).await,
                Err(DriverError::Server { .. })
            ),
            "{sql}"
        );
    }
    assert!(matches!(
        connection
            .execute("SELECT 1; ATTACH 'x.duckdb' AS x", &mut DiscardSink)
            .await,
        Err(DriverError::Unsupported(_))
    ));
    assert!(
        matches!(connection.execute("SELEC 1", &mut DiscardSink).await, Err(DriverError::Server { code, message }) if code == "42601" && message.contains("Parser Error"))
    );
    connection.ping().await?;
    Box::new(connection).close().await?;
    Ok(())
}
#[tokio::test]
async fn fidelity_native_and_explicit_text_fallbacks() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = open(memory("type-test")).await?;
    for &(expression, expected) in TYPE_CASES {
        let mut sink = Collect::default();
        connection
            .execute(&format!("SELECT {expression} AS value"), &mut sink)
            .await?;
        let options = arrow_cast::display::FormatOptions::default().with_null("NULL");
        let formatter = arrow_cast::display::ArrayFormatter::try_new(
            sink.batches[0].column(0).as_ref(),
            &options,
        )?;
        assert_eq!(
            formatter.value(0).to_string(),
            expected,
            "{expression}: {:?}",
            sink.schemas[0].field(0)
        );
        if expression.starts_with("UUID ") {
            let field = sink.schemas[0].field(0);
            assert_eq!(
                field.data_type(),
                &arrow_schema::DataType::FixedSizeBinary(16)
            );
            assert_eq!(
                field.metadata()[openrows_driver_api::metadata::EXTENSION_NAME],
                openrows_driver_api::metadata::UUID_EXTENSION
            );
        }
    }
    Box::new(connection).close().await?;
    Ok(())
}

#[tokio::test]
async fn ordered_query_and_early_stop_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let mut connection = open(memory("ordered-stop-test")).await?;
    let digits = "(SELECT 0 AS d UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL SELECT 7 UNION ALL SELECT 8 UNION ALL SELECT 9)";
    let series = format!(
        "SELECT a.d + 10*b.d + 100*c.d + 1000*e.d + 1 AS n FROM {digits} a CROSS JOIN {digits} b CROSS JOIN {digits} c CROSS JOIN {digits} e ORDER BY n"
    );
    for sql in [
        "SELECT i FROM range(10000) t(i) ORDER BY i",
        series.as_str(),
    ] {
        let mut all = Collect::default();
        let summary = connection
            .execute(sql, &mut all)
            .await
            .map_err(|e| format!("complete {sql}: {e}"))?;
        assert_eq!(summary.rows, 10000);
        let mut stopped = Collect {
            stop: true,
            ..Collect::default()
        };
        connection
            .execute(sql, &mut stopped)
            .await
            .map_err(|e| format!("stop {sql}: {e}"))?;
        assert_eq!(stopped.batches.len(), 1);
        connection.ping().await?;
    }
    Box::new(connection).close().await?;
    Ok(())
}

const TYPE_CASES: &[(&str, &str)] = &[
    ("CAST(-128 AS TINYINT)", "-128"),
    ("255::UTINYINT", "255"),
    ("65535::USMALLINT", "65535"),
    ("4294967295::UINTEGER", "4294967295"),
    ("18446744073709551615::UBIGINT", "18446744073709551615"),
    (
        "'-170141183460469231731687303715884105728'::HUGEINT",
        "-170141183460469231731687303715884105728",
    ),
    (
        "'340282366920938463463374607431768211455'::UHUGEINT",
        "340282366920938463463374607431768211455",
    ),
    ("12.50::DECIMAL(10,2)", "12.50"),
    ("DATE '2024-05-06'", "2024-05-06"),
    ("TIME '13:05:00.25'", "13:05:00.250"),
    ("TIMESTAMP '2024-05-06 13:05:00'", "2024-05-06T13:05:00"),
    (
        "TIMESTAMPTZ '2024-05-06 13:05:00+00'",
        "2024-05-06T13:05:00Z",
    ),
    (
        "INTERVAL '1 month 2 days 3 microseconds'",
        "1 mons 2 days 0.000003000 secs",
    ),
    (
        "UUID '00112233-4455-6677-8899-aabbccddeeff'",
        "00112233445566778899aabbccddeeff",
    ),
    ("from_hex('00ff10')", "00ff10"),
    ("[1, NULL, 3]", "[1, NULL, 3]"),
    ("[true, NULL, false]", "[true, NULL, false]"),
    (
        "{'h': '340282366920938463463374607431768211455'::UHUGEINT}",
        "{h: 340282366920938463463374607431768211455}",
    ),
    ("{'a': 1, 'b': 'x'}", "{a: 1, b: x}"),
    ("MAP(['a'], [1])", "{a: 1}"),
    ("union_value(x := 1)", "{x=1}"),
    ("'happy'::ENUM('sad', 'happy')", "happy"),
];
