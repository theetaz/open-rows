// SPDX-License-Identifier: Apache-2.0
//! At most four user-triggered turns; cancellation is keyed by tab-owned request ID.
use crate::connections::IpcError;
use openrows_ai::Cancellation;
use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};
#[derive(Default)]
pub(crate) struct AssistantState(Mutex<HashMap<String, Cancellation>>);
pub(super) struct Run<'a> {
    state: &'a AssistantState,
    id: String,
}
impl AssistantState {
    pub(super) fn start(&self, id: &str) -> Result<(Run<'_>, Cancellation), IpcError> {
        openrows_ai::validate_id(id)?;
        let mut runs = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if runs.len() >= 4 || runs.contains_key(id) {
            return Err(IpcError::invalid(
                "Assistant already running or capacity reached",
            ));
        }
        let cancel = Cancellation::default();
        runs.insert(id.into(), cancel.clone());
        Ok((
            Run {
                state: self,
                id: id.into(),
            },
            cancel,
        ))
    }
    pub(crate) fn cancel(&self, id: Option<&str>) {
        for (key, cancel) in self.0.lock().unwrap_or_else(PoisonError::into_inner).iter() {
            if id.is_none_or(|id| id == key) {
                cancel.cancel();
            }
        }
    }
}
impl Drop for Run<'_> {
    fn drop(&mut self) {
        self.state
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capacity_drop_and_scope_cancellation_are_bounded() {
        let state = AssistantState::default();
        let runs: Vec<_> = (0..4)
            .map(|index| state.start(&format!("run-{index}")))
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(state.start("extra").is_err());
        state.cancel(Some("run-0"));
        assert!(cancelled(&runs[0].1));
        assert!(!cancelled(&runs[1].1));
        state.cancel(None);
        assert!(runs.iter().all(|(_, cancel)| cancelled(cancel)));
        drop(runs);
        assert!(state.start("extra").is_ok());
    }
    fn cancelled(cancel: &Cancellation) -> bool {
        use std::future::Future;
        let mut future = std::pin::pin!(cancel.cancelled());
        future
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
            .is_ready()
    }
}
