// SPDX-License-Identifier: Apache-2.0
//! Cancellation is local request abort, followed by best-effort baton close.
use openrows_driver_api::{BoxFuture, CancelHandle, DriverError};
use tokio::sync::watch;

pub(crate) struct Abort(pub(crate) watch::Sender<u64>);

impl Abort {
    pub(crate) fn new() -> Self {
        let (sender, _) = watch::channel(0);
        Self(sender)
    }
}

impl CancelHandle for Abort {
    fn cancel(&self) -> BoxFuture<'static, Result<(), DriverError>> {
        self.0
            .send_modify(|generation| *generation = generation.wrapping_add(1));
        Box::pin(async { Ok(()) })
    }
}
