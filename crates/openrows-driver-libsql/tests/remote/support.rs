// SPDX-License-Identifier: Apache-2.0
//! Synthetic authenticated request and a collecting result sink.
use arrow_array::{Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, ConnectTrace, DriverError, Endpoint, ResultSink, SinkFlow,
    SslMode, TlsFiles,
};
use openrows_driver_libsql::{LibsqlSession, connect};
use openrows_net::SecretString;

pub(super) fn request() -> Result<ConnectRequest, DriverError> {
    let variable = |name: &str| {
        std::env::var(name)
            .map_err(|_| DriverError::Unsupported(format!("{name} is unset; run just test-libsql")))
    };
    let port = variable("OPENROWS_TEST_LIBSQL_PORT")?
        .parse()
        .map_err(|_| DriverError::Unsupported("invalid test port".into()))?;
    Ok(ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".into(),
            port,
        },
        database: "main".into(),
        user: String::new(),
        password: Some(SecretString::from(
            variable("OPENROWS_TEST_LIBSQL_TOKEN")?.as_str(),
        )),
        application_name: "test".into(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        driver_options: std::collections::BTreeMap::new(),
        startup_commands: Vec::new(),
    })
}
pub(super) async fn session() -> Result<LibsqlSession, DriverError> {
    connect(&request()?, &mut ConnectTrace::default()).await
}
#[derive(Default)]
pub(super) struct Collect {
    pub(super) batches: Vec<RecordBatch>,
    pub(super) notices: Vec<String>,
}
impl Collect {
    pub(super) fn value(&self, row: usize, column: usize) -> Option<String> {
        let batch = self.batches.first()?;
        let array = batch.column(column);
        if array.is_null(row) {
            return None;
        }
        if let Some(array) = array.as_any().downcast_ref::<StringArray>() {
            return Some(array.value(row).into());
        }
        array
            .as_any()
            .downcast_ref::<Int64Array>()
            .map(|array| array.value(row).to_string())
    }
}
impl ResultSink for Collect {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.batches.push(batch);
        SinkFlow::Continue
    }
    fn notice(&mut self, _level: openrows_driver_api::NoticeLevel, message: &str) {
        self.notices.push(message.into());
    }
}
