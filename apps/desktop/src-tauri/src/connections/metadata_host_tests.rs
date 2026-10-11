// SPDX-License-Identifier: Apache-2.0
//! Idle metadata loss invalidates only its original slot and never reconnects.
use super::*;
use openrows_driver_api::{BoxFuture, ExecSummary, ResultSink, ServerSummary, Session, TlsStatus};
struct Hosted {
    failed: Arc<tokio::sync::Notify>,
    summary: ServerSummary,
}
impl Session for Hosted {
    fn execute<'a>(
        &'a mut self,
        _sql: &'a str,
        _sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(async { Ok(ExecSummary::default()) })
    }
    fn host_failure(&self) -> Option<BoxFuture<'static, DriverError>> {
        let failed = self.failed.clone();
        Some(Box::pin(async move {
            failed.notified().await;
            DriverError::Host(openrows_driver_api::HostFailure::Exited)
        }))
    }
    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        Box::pin(async { Ok(()) })
    }
}
impl Connection for Hosted {
    fn server(&self) -> &ServerSummary {
        &self.summary
    }
}
fn held(slot: &Slot, failed: Arc<tokio::sync::Notify>) -> Held {
    let connection = Box::new(Hosted {
        failed,
        summary: ServerSummary {
            product: "test-only".into(),
            version: "1".into(),
            flavour: None,
            database: "main".into(),
            user: String::new(),
            tls: TlsStatus::LocalSocket,
        },
    });
    let lifecycle =
        super::super::metadata_lifecycle::Observer::new(connection.as_ref(), Arc::downgrade(slot));
    Held {
        connection,
        used: Instant::now(),
        lifecycle,
    }
}
#[tokio::test]
async fn metadata_host_loss_invalidates_just_that_slot() {
    let sessions = MetadataSessions::default();
    let a = sessions.slot(("a".into(), "main".into()));
    let b = sessions.slot(("b".into(), "main".into()));
    let failed = Arc::new(tokio::sync::Notify::new());
    *a.lock().await = Some(held(&a, failed.clone()));
    *b.lock().await = Some(held(&b, Arc::new(tokio::sync::Notify::new())));
    failed.notify_one();
    tokio::time::timeout(Duration::from_secs(2), async {
        while a.lock().await.is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(b.lock().await.is_some());
    assert!(Arc::ptr_eq(&a, &sessions.slot(("a".into(), "main".into()))));
}

#[tokio::test]
async fn cancelled_metadata_read_discards_only_its_connection() {
    let sessions = MetadataSessions::default();
    let a = sessions.slot(("a".into(), "main".into()));
    let b = sessions.slot(("b".into(), "main".into()));
    *a.lock().await = Some(held(&a, Arc::new(tokio::sync::Notify::new())));
    *b.lock().await = Some(held(&b, Arc::new(tokio::sync::Notify::new())));
    let entered = Arc::new(tokio::sync::Notify::new());
    let notified = entered.notified();
    let producer = entered.clone();
    let mut read = Box::pin(sessions.read(
        ("a", "main"),
        || async { Err(IpcError::new(IpcErrorKind::Internal, "not used")) },
        move |_: &mut dyn Connection| {
            Box::pin(async move {
                producer.notify_one();
                std::future::pending::<Result<(), DriverError>>().await
            })
        },
    ));
    tokio::select! {
        () = notified => {},
        result = &mut read => panic!("read unexpectedly completed: {result:?}"),
    }
    drop(read);
    assert!(a.lock().await.is_none());
    assert!(b.lock().await.is_some());
}
