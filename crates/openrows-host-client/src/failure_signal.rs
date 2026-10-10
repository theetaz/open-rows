// SPDX-License-Identifier: Apache-2.0
//! Permanent failure is latched before EOF; queue saturation cannot drop cleanup.
use crate::{HostFailure, session::Shutdown};
use std::sync::Mutex;
use tokio::{runtime::Handle, sync::mpsc};
pub(crate) struct FailureSignal {
    reason: Mutex<Option<HostFailure>>,
    shutdown: mpsc::Sender<Shutdown>,
    runtime: Handle,
}
impl FailureSignal {
    pub(crate) fn new(shutdown: mpsc::Sender<Shutdown>) -> Self {
        Self {
            reason: Mutex::new(None),
            shutdown,
            runtime: Handle::current(),
        }
    }
    pub(crate) fn current(&self) -> Result<Option<HostFailure>, HostFailure> {
        self.reason
            .lock()
            .map(|reason| *reason)
            .map_err(|_| HostFailure::Protocol)
    }
    pub(crate) fn record(&self, failure: HostFailure) -> bool {
        let Ok(mut reason) = self.reason.lock() else {
            return true;
        };
        if reason.is_some() {
            return false;
        }
        *reason = Some(failure);
        true
    }
    pub(crate) fn fail(&self, failure: HostFailure) {
        if !self.record(failure) {
            return;
        }
        if let Err(mpsc::error::TrySendError::Full(message)) =
            self.shutdown.try_send(Shutdown::Failure(failure))
        {
            let shutdown = self.shutdown.clone();
            drop(self.runtime.spawn(async move {
                let _ = shutdown.send(message).await;
            }));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn full_supervisor_queue_keeps_one_failure_signal_and_its_original_reason()
    -> Result<(), HostFailure> {
        let (send, mut receive) = mpsc::channel(1);
        send.try_send(Shutdown::Completed(1))
            .map_err(|_| HostFailure::Protocol)?;
        let signal = FailureSignal::new(send);
        signal.fail(HostFailure::Protocol);
        signal.fail(HostFailure::Exited);
        assert_eq!(signal.current()?, Some(HostFailure::Protocol));
        assert!(matches!(receive.recv().await, Some(Shutdown::Completed(1))));
        assert!(matches!(
            receive.recv().await,
            Some(Shutdown::Failure(HostFailure::Protocol))
        ));
        assert!(receive.try_recv().is_err());
        Ok(())
    }
}
