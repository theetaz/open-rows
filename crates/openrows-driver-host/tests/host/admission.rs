// SPDX-License-Identifier: Apache-2.0
//! A repeated Pause cannot authorize a batch; cancellation still wakes the proxy.
use crate::{adapter, support};
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{BoxFuture, DriverError, ResultSink, SinkAdmission, SinkFlow};
use std::{sync::Arc, time::Duration};

struct Sink {
    rows: usize,
    waits: usize,
    waiting: Arc<tokio::sync::Notify>,
}
impl ResultSink for Sink {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, batch: RecordBatch) -> SinkFlow {
        self.rows += batch.num_rows();
        SinkFlow::Continue
    }
    fn admission(&self) -> SinkAdmission {
        if self.rows == 0 {
            SinkAdmission::Ready
        } else {
            SinkAdmission::Pause
        }
    }
    fn resume(&mut self) -> BoxFuture<'_, SinkAdmission> {
        self.waits += 1;
        if self.waits == 1 {
            Box::pin(std::future::ready(SinkAdmission::Pause))
        } else {
            self.waiting.notify_one();
            Box::pin(std::future::pending())
        }
    }
}
#[tokio::test]
async fn repeated_pause_withholds_credit_until_ready_and_keeps_cancel_live() {
    let _permit = support::host_test().await.unwrap();
    let mut connection = adapter::connection().await;
    let cancel = connection.cancel_handle().unwrap();
    let waiting = Arc::new(tokio::sync::Notify::new());
    let mut sink = Sink {
        rows: 0,
        waits: 0,
        waiting: waiting.clone(),
    };
    let sql = "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<10000) SELECT x FROM n";
    let mut execution = connection.execute(sql, &mut sink);
    tokio::select! {
        () = waiting.notified() => {},
        result = &mut execution => panic!("early completion {result:?}"),
    }
    cancel.cancel().await.unwrap();
    let error = tokio::time::timeout(Duration::from_millis(200), execution)
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(error, DriverError::Server { ref code, .. } if code == "57014"));
    assert_eq!(sink.rows, 200, "Pause must not admit the second batch");
    connection.ping().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn refused_exclusive_credit_poisons_only_its_owner_and_preserves_memory_reason()
-> Result<(), openrows_host_client::HostFailure> {
    use openrows_host_client::{HostFailure, MemoryBudget};
    use openrows_host_protocol::Control;
    let _permit = support::host_test().await?;
    let memory = MemoryBudget::default();
    let mut survivor = support::host(memory.clone()).await?;
    let mut victim = support::host(memory.clone()).await?;
    let pid = victim.pid();
    let directory = victim.directory().to_path_buf();
    victim.send(
        2,
        &Control::Execute {
            sql: "select hex(zeroblob(2200000))".into(),
        },
    )?;
    loop {
        if let Control::BatchReady(grant) = support::receive(&mut victim).await?.1 {
            assert!(grant.exclusive);
            assert_eq!(
                victim.send(0, &Control::Grant(grant)),
                Err(HostFailure::MemoryLimit)
            );
            break;
        }
    }
    assert_eq!(
        victim.send(3, &Control::Ping),
        Err(HostFailure::MemoryLimit)
    );
    assert_eq!(support::failed(&victim).await, HostFailure::MemoryLimit);
    support::assert_reaped(pid, &directory);
    assert_eq!(memory.reserved(), openrows_host_client::HOST_MEMORY);
    assert_eq!(
        support::execute(&mut survivor, 2, "select 1").await?.rows,
        1
    );
    survivor.close().await?;
    assert_eq!(memory.reserved(), 0);
    Ok(())
}
