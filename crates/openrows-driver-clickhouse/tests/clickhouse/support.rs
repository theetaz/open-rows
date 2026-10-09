// SPDX-License-Identifier: Apache-2.0
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, Endpoint, ResultSink, SinkFlow, SslMode, TlsFiles,
};
use openrows_net::SecretString;
pub type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
pub fn config() -> Result<ConnectRequest, Box<dyn std::error::Error + Send + Sync>> {
    Ok(ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".to_owned(),
            port: std::env::var("OPENROWS_TEST_CLICKHOUSE_PORT")?.parse()?,
        },
        database: "openrows_test".to_owned(),
        user: "default".to_owned(),
        password: Some(SecretString::from(std::env::var(
            "OPENROWS_TEST_CLICKHOUSE_PASSWORD",
        )?)),
        application_name: "OpenRows test".to_owned(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
    })
}
#[derive(Default)]
pub struct Collect {
    pub batches: Vec<RecordBatch>,
    pub schema: Option<SchemaRef>,
    pub stop: bool,
}
impl ResultSink for Collect {
    fn begin_result(&mut self, schema: SchemaRef) {
        self.schema = Some(schema);
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
