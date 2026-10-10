// SPDX-License-Identifier: Apache-2.0
//! Idle host failure invalidates only the matching metadata slot, without reconnect.
use super::metadata::Slot;
use openrows_driver_api::Connection;
use std::sync::{Arc, Weak};

pub(super) struct Observer {
    identity: Arc<()>,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Observer {
    pub(super) fn new(
        connection: &dyn Connection,
        slot: Weak<tokio::sync::Mutex<Option<super::metadata::Held>>>,
    ) -> Self {
        let identity = Arc::new(());
        let expected = identity.clone();
        let task = connection
            .host_failure()
            .map(|failure| tokio::spawn(observe(failure, slot, expected)));
        Self { identity, task }
    }
}
async fn observe(
    failure: openrows_driver_api::BoxFuture<'static, openrows_driver_api::DriverError>,
    slot: Weak<tokio::sync::Mutex<Option<super::metadata::Held>>>,
    expected: Arc<()>,
) {
    let _ = failure.await;
    if let Some(slot) = slot.upgrade() {
        invalidate(&slot, &expected).await;
    }
}
async fn invalidate(slot: &Slot, expected: &Arc<()>) {
    let mut held = slot.lock().await;
    if held
        .as_ref()
        .is_some_and(|held| Arc::ptr_eq(&held.lifecycle.identity, expected))
    {
        held.take();
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
