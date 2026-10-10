// SPDX-License-Identifier: Apache-2.0
//! Real SQL producer and a counting sink shared by the adapter measurements.
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, Endpoint, FileSource, SslMode, TlsFiles,
};
use openrows_driver_api::{ResultSink, SinkFlow};
use openrows_host_client::{
    CircuitBreaker, HostSupervisor, HostedConnection, HostedDriver, LaunchOptions, MemoryBudget,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
pub(super) fn enabled() -> bool {
    std::env::var("OPENROWS_BENCH").as_deref() == Ok("1")
}
pub(super) fn request() -> ConnectRequest {
    ConnectRequest {
        endpoint: Endpoint::File {
            source: FileSource::Memory {
                name: "test-only".into(),
            },
            attached: vec![],
        },
        database: "main".into(),
        user: String::new(),
        password: None,
        application_name: "test-only".into(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        driver_options: BTreeMap::new(),
        startup_commands: vec![],
    }
}
pub(super) fn supervisor() -> Result<HostSupervisor, Box<dyn std::error::Error>> {
    Ok(HostSupervisor::new(LaunchOptions {
        driver: openrows_host_client::HostDriverId::SqliteTest,
        executable: PathBuf::from(std::env::var("OPENROWS_HOST_BIN")?).canonicalize()?,
        endpoint_root: if cfg!(unix) {
            "/tmp".into()
        } else {
            std::env::temp_dir()
        },
        memory: MemoryBudget::default(),
        circuit: CircuitBreaker::default(),
    }))
}
pub(super) async fn hosted() -> Result<HostedConnection, Box<dyn std::error::Error>> {
    Ok(HostedDriver::new(
        supervisor()?,
        Arc::new(openrows_driver_sqlite::SqliteDriver),
    )
    .open(
        &request(),
        &mut openrows_driver_api::ConnectTrace::default(),
    )
    .await?)
}
pub(super) fn sql(rows: usize, projection: &str) -> String {
    format!(
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<{rows}) SELECT {projection} FROM n"
    )
}
#[derive(Default)]
pub(super) struct Count {
    pub(super) rows: usize,
}
impl ResultSink for Count {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.rows += batch.num_rows();
        std::hint::black_box(batch);
        SinkFlow::Continue
    }
}

pub(super) fn timer_resolution() -> std::time::Duration {
    (0..1000)
        .filter_map(|_| {
            let now = std::time::Instant::now();
            let elapsed = now.elapsed();
            (!elapsed.is_zero()).then_some(elapsed)
        })
        .min()
        .unwrap_or_default()
}
