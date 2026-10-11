// SPDX-License-Identifier: Apache-2.0
//! Shared native database state-verification fixtures.
#![allow(
    dead_code,
    reason = "database fixtures use either file or network connection constructors"
)]
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, DiscardSink, Driver, Endpoint, FileSource,
    ResultSink, SinkFlow, SslMode, TlsFiles,
};
use openrows_mcp::{ReadEngine, protected_read};
use std::path::Path;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const ATTACKS: &[&str] = &[
    "DELETE FROM ai_fixture",
    "DROP TABLE ai_fixture",
    "WITH x AS (DELETE FROM ai_fixture RETURNING *) SELECT * FROM x",
    "SELECT * INTO other FROM ai_fixture",
    "SELECT pg_terminate_backend(1)",
    "SELECT pg_catalog.pg_terminate_backend(1)",
    "SELECT nextval('x')",
    "SELECT load_extension('x')",
    "COPY ai_fixture TO PROGRAM 'x'",
    "SELECT 1; /* comment */ DELETE FROM ai_fixture",
    "SELECT /*!50000 get_lock('x',1) */ 1",
    "SELECT * FROM main.ai_fixture FOR UPDATE",
    "SELECT email hidden FROM main.ai_fixture",
    "SELECT * FROM main.ai_view",
    "SELECT id FROM main.ai_fixture WHERE id !! 1",
    "SELECT id FROM main.ai_fixture WHERE id ** 1",
];
pub async fn exercise(
    driver: &dyn Driver,
    request: &ConnectRequest,
    engine: ReadEngine,
    schema: &str,
) -> TestResult {
    let mut admin = driver
        .connect(request, &mut ConnectTrace::default())
        .await?;
    initialize(admin.as_mut()).await?;
    verify_attacks(driver, request, (engine, schema), admin.as_mut()).await?;
    verify_engine_mode(driver, request, engine, admin.as_mut()).await?;
    let connection = driver
        .connect(request, &mut ConnectTrace::default())
        .await?;
    let bundle = protected_read(
        connection,
        engine,
        &format!("SELECT * FROM {schema}.ai_fixture"),
        false,
    )
    .await?;
    assert_eq!(bundle.samples[0].rows.len(), 1);
    let connection = driver
        .connect(request, &mut ConnectTrace::default())
        .await?;
    let plan = protected_read(
        connection,
        engine,
        &format!("SELECT * FROM {schema}.ai_fixture"),
        true,
    )
    .await?;
    assert!(!plan.plan.is_empty());
    admin.execute("DROP VIEW ai_view", &mut DiscardSink).await?;
    admin
        .execute("DROP TABLE ai_fixture", &mut DiscardSink)
        .await?;
    admin.close().await?;
    Ok(())
}
async fn verify_attacks(
    driver: &dyn Driver,
    request: &ConnectRequest,
    (engine, schema): (ReadEngine, &str),
    admin: &mut dyn openrows_driver_api::Connection,
) -> TestResult {
    for attack in ATTACKS {
        let attack = attack.replace("main.", &format!("{schema}."));
        let connection = driver
            .connect(request, &mut ConnectTrace::default())
            .await?;
        assert!(
            protected_read(connection, engine, &attack, false)
                .await
                .is_err(),
            "{engine:?}: {attack}"
        );
        let mut rows = Collect::default();
        admin
            .execute("SELECT id FROM ai_fixture", &mut rows)
            .await?;
        assert_eq!(rows.values, vec!["1"], "state changed after {attack}");
    }
    Ok(())
}
async fn verify_engine_mode(
    driver: &dyn Driver,
    request: &ConnectRequest,
    engine: ReadEngine,
    admin: &mut dyn openrows_driver_api::Connection,
) -> TestResult {
    let connection = driver
        .connect(request, &mut ConnectTrace::default())
        .await?;
    assert!(
        protected_read(Box::new(InjectWrite(connection)), engine, "SELECT 1", false)
            .await
            .is_err()
    );
    let mut rows = Collect::default();
    admin
        .execute("SELECT id FROM ai_fixture", &mut rows)
        .await?;
    assert_eq!(
        rows.values,
        vec!["1"],
        "engine read-only mode allowed a write"
    );
    Ok(())
}
async fn initialize(admin: &mut dyn openrows_driver_api::Connection) -> TestResult {
    admin
        .execute(
            "CREATE TABLE ai_fixture (id INTEGER, email TEXT)",
            &mut DiscardSink,
        )
        .await?;
    admin
        .execute(
            "INSERT INTO ai_fixture VALUES (1,'test-only')",
            &mut DiscardSink,
        )
        .await?;
    admin
        .execute(
            "CREATE VIEW ai_view AS SELECT * FROM ai_fixture",
            &mut DiscardSink,
        )
        .await?;
    Ok(())
}
/// Prove engine protection itself rejects a write, independently of the SQL validator.
struct InjectWrite(Box<dyn openrows_driver_api::Connection>);
impl openrows_driver_api::Session for InjectWrite {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> openrows_driver_api::BoxFuture<
        'a,
        Result<openrows_driver_api::ExecSummary, openrows_driver_api::DriverError>,
    > {
        self.0.execute(
            if sql == "SELECT 1" {
                "DELETE FROM ai_fixture"
            } else {
                sql
            },
            sink,
        )
    }
    fn cancel_handle(&self) -> Option<std::sync::Arc<dyn openrows_driver_api::CancelHandle>> {
        self.0.cancel_handle()
    }
    fn close(
        self: Box<Self>,
    ) -> openrows_driver_api::BoxFuture<'static, Result<(), openrows_driver_api::DriverError>> {
        self.0.close()
    }
}
impl openrows_driver_api::Connection for InjectWrite {
    fn server(&self) -> &openrows_driver_api::ServerSummary {
        self.0.server()
    }
}
pub fn file_request(path: &Path) -> ConnectRequest {
    let mut request = base_request();
    request.endpoint = Endpoint::File {
        source: FileSource::Disk {
            path: path.into(),
            read_only: false,
            create: true,
        },
        attached: vec![],
    };
    request.database = "main".into();
    request
}
pub fn server_request(
    port: &str,
    password: &str,
    user: &str,
    database: &str,
) -> TestResult<ConnectRequest> {
    let mut request = base_request();
    request.endpoint = Endpoint::Server {
        host: "127.0.0.1".into(),
        port: std::env::var(port)?.parse()?,
    };
    request.password = Some(openrows_net::SecretString::from(
        std::env::var(password)?.as_str(),
    ));
    request.user = user.into();
    request.database = database.into();
    Ok(request)
}
fn base_request() -> ConnectRequest {
    ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".into(),
            port: 1,
        },
        database: String::new(),
        user: String::new(),
        password: None,
        application_name: "mcp-test".into(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: vec![],
        driver_options: std::collections::BTreeMap::default(),
    }
}
#[derive(Default)]
struct Collect {
    values: Vec<String>,
}
impl ResultSink for Collect {
    fn begin_result(&mut self, _: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        for row in 0..batch.num_rows() {
            self.values.push(
                arrow_cast::display::array_value_to_string(batch.column(0), row)
                    .unwrap_or_default(),
            );
        }
        SinkFlow::Continue
    }
}
