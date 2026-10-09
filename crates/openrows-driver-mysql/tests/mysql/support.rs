// SPDX-License-Identifier: Apache-2.0
//! Connection settings from `scripts/test-mysql.sh` and a collecting sink.
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, DriverError, Endpoint, NoticeLevel, ResultSink,
    Session, SinkFlow, SslMode, TlsFiles,
};
use openrows_driver_mysql::{MySqlSession, connect};
use openrows_net::SecretString;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The database the script creates.
pub const DATABASE: &str = "openrows_test";

fn env(name: &str) -> Result<String, DriverError> {
    std::env::var(name)
        .map_err(|_| DriverError::Unsupported(format!("{name} is not set; run `just test-mysql`")))
}

/// `true` when the script started `MariaDB`.
pub fn mariadb() -> bool {
    std::env::var("OPENROWS_TEST_MYSQL_ENGINE").is_ok_and(|engine| engine == "mariadb")
}

/// A request for the test server as `root`.
pub fn request(ssl_mode: SslMode) -> Result<ConnectRequest, DriverError> {
    let port = env("OPENROWS_TEST_MYSQL_PORT")?
        .parse()
        .map_err(|_| DriverError::Unsupported("bad port".to_owned()))?;
    Ok(ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port,
        },
        database: DATABASE.to_owned(),
        user: "root".to_owned(),
        password: Some(SecretString::from(
            env("OPENROWS_TEST_MYSQL_PASSWORD")?.as_str(),
        )),
        application_name: "openrows-test".to_owned(),
        ssl_mode,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
    })
}

/// A connected session (`sslmode=prefer`).
pub async fn session() -> Result<MySqlSession, DriverError> {
    connect(&request(SslMode::Prefer)?, &mut ConnectTrace::default()).await
}

/// Collects batches and notices.
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
pub async fn run(session: &mut MySqlSession, statements: &[&str]) -> Result<(), DriverError> {
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
