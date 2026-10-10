// SPDX-License-Identifier: Apache-2.0
//! Connection settings from `scripts/test-mssql.sh` and a collecting sink.
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, DriverError, Endpoint, NoticeLevel, ResultSink,
    Session, SinkFlow, SslMode, TlsFiles,
};
use openrows_driver_mssql::{SqlServerSession, connect};
use openrows_net::SecretString;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The database the script creates.
pub const DATABASE: &str = "openrows_test";

fn env(name: &str) -> Result<String, DriverError> {
    std::env::var(name)
        .map_err(|_| DriverError::Unsupported(format!("{name} is not set; run `just test-mssql`")))
}

/// A request for the test server as `sa`.
pub fn request(ssl_mode: SslMode) -> Result<ConnectRequest, DriverError> {
    let port = env("OPENROWS_TEST_MSSQL_PORT")?
        .parse()
        .map_err(|_| DriverError::Unsupported("bad port".to_owned()))?;
    Ok(ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port,
        },
        database: DATABASE.to_owned(),
        user: "sa".to_owned(),
        password: Some(SecretString::from(
            env("OPENROWS_TEST_MSSQL_PASSWORD")?.as_str(),
        )),
        application_name: "openrows-test".to_owned(),
        ssl_mode,
        tls: TlsFiles::default(),
        via: None,
        // The server runs emulated on some machines: allow slow logins.
        timeouts: ConnectTimeouts {
            auth: std::time::Duration::from_secs(60),
            ..ConnectTimeouts::default()
        },
        driver_options: std::collections::BTreeMap::new(),
        startup_commands: Vec::new(),
    })
}

/// A connected session (`sslmode=require`: the server's certificate is
/// self-signed).
pub async fn session() -> Result<SqlServerSession, DriverError> {
    connect(&request(SslMode::Require)?, &mut ConnectTrace::default()).await
}

/// Collects results, batches and notices.
#[derive(Default)]
pub struct Collect {
    pub schemas: Vec<SchemaRef>,
    pub batches: Vec<RecordBatch>,
    pub notices: Vec<(NoticeLevel, String)>,
}

impl ResultSink for Collect {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.schemas.push(schema);
    }

    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.batches.push(batch);
        SinkFlow::Continue
    }

    fn notice(&mut self, level: NoticeLevel, message: &str) {
        self.notices.push((level, message.to_owned()));
    }
}

impl Collect {
    pub fn rows(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }
}

/// Run statements one by one, discarding rows.
pub async fn run(session: &mut SqlServerSession, statements: &[&str]) -> Result<(), DriverError> {
    for sql in statements {
        session.execute(sql, &mut Collect::default()).await?;
    }
    Ok(())
}

/// The SQLSTATE of a server error, or a description of anything else.
pub fn code<T>(result: Result<T, DriverError>) -> String {
    match result {
        Err(DriverError::Server { code, .. }) => code,
        Err(other) => format!("{other:?}"),
        Ok(_) => "ok".to_owned(),
    }
}

/// A fixture name unique to this test process.
pub fn fixture(name: &str) -> String {
    format!("orx_{name}_{}", std::process::id())
}
