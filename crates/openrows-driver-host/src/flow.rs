// SPDX-License-Identifier: Apache-2.0
//! Only the dedicated worker waits for credit; control uses short state transitions.
use openrows_host_protocol::{CreditState, FlowKey, Grant, ProtocolError};
use std::sync::{Condvar, Mutex};
#[derive(Default)]
struct State {
    credit: CreditState,
    request: u64,
    cancelled: bool,
    facet: u64,
    acknowledged: u64,
    closing: bool,
}
#[derive(Default)]
pub(crate) struct Flow {
    state: Mutex<State>,
    changed: Condvar,
}
impl Flow {
    pub(crate) fn begin(&self, request: u64) -> Result<(), ProtocolError> {
        let mut state = self.state.lock().map_err(|_| ProtocolError::Invalid)?;
        state.credit = CreditState::default();
        state.request = request;
        state.cancelled = false;
        state.facet = 0;
        state.acknowledged = 0;
        Ok(())
    }
    pub(crate) fn offer(&self, grant: Grant) -> Result<(), ProtocolError> {
        self.state
            .lock()
            .map_err(|_| ProtocolError::Invalid)?
            .credit
            .offer(grant)
    }
    pub(crate) fn authorize(&self) -> Result<bool, ProtocolError> {
        let mut state = self.state.lock().map_err(|_| ProtocolError::Invalid)?;
        while !state.credit.authorized()
            && !state.credit.stopped()
            && !state.cancelled
            && !state.closing
        {
            state = self
                .changed
                .wait(state)
                .map_err(|_| ProtocolError::Invalid)?;
        }
        if state.cancelled || state.closing || state.credit.stopped() {
            return Ok(false);
        }
        state.credit.sent()?;
        Ok(true)
    }
    pub(crate) fn consumed(&self) -> Result<bool, ProtocolError> {
        let mut state = self.state.lock().map_err(|_| ProtocolError::Invalid)?;
        while state.credit.pending()
            && !state.credit.stopped()
            && !state.cancelled
            && !state.closing
        {
            state = self
                .changed
                .wait(state)
                .map_err(|_| ProtocolError::Invalid)?;
        }
        Ok(!state.credit.stopped() && !state.cancelled && !state.closing)
    }
    pub(crate) fn apply(
        &self,
        control: &openrows_host_protocol::Control,
    ) -> Result<(), ProtocolError> {
        use openrows_host_protocol::Control;
        let mut state = self.state.lock().map_err(|_| ProtocolError::Invalid)?;
        let key = match control {
            Control::Grant(grant) | Control::Resume(grant) => grant.key,
            Control::Consumed(key) | Control::Pause(key) | Control::Stop(key) => *key,
            _ => return Err(ProtocolError::Invalid),
        };
        if key.request < state.request {
            return Ok(());
        }
        match control {
            Control::Grant(grant) | Control::Resume(grant) => {
                check_request(state.request, grant.key)?;
                state
                    .credit
                    .grant(*grant, matches!(control, Control::Resume(_)))?;
            }
            Control::Consumed(key) => {
                check_request(state.request, *key)?;
                state.credit.consume(*key)?;
            }
            Control::Pause(key) => {
                check_request(state.request, *key)?;
                state.credit.pause();
            }
            Control::Stop(key) => {
                check_request(state.request, *key)?;
                state.credit.stop();
            }
            _ => return Err(ProtocolError::Invalid),
        }
        self.changed.notify_all();
        Ok(())
    }
    pub(crate) fn offer_facet(&self, sequence: u64) -> Result<(), ProtocolError> {
        let mut state = self.state.lock().map_err(|_| ProtocolError::Invalid)?;
        if sequence != state.facet + 1 || state.facet != state.acknowledged {
            return Err(ProtocolError::Stale);
        }
        state.facet = sequence;
        Ok(())
    }
    pub(crate) fn acknowledge_facet(
        &self,
        request: u64,
        sequence: u64,
    ) -> Result<(), ProtocolError> {
        let mut state = self.state.lock().map_err(|_| ProtocolError::Invalid)?;
        if request < state.request {
            return Ok(());
        }
        if request != state.request || sequence > state.facet || sequence == 0 {
            return Err(ProtocolError::Stale);
        }
        state.acknowledged = state.acknowledged.max(sequence);
        self.changed.notify_all();
        Ok(())
    }
    pub(crate) fn wait_facet(&self) -> Result<bool, ProtocolError> {
        let mut state = self.state.lock().map_err(|_| ProtocolError::Invalid)?;
        while state.facet != state.acknowledged && !state.cancelled && !state.closing {
            state = self
                .changed
                .wait(state)
                .map_err(|_| ProtocolError::Invalid)?;
        }
        Ok(!state.cancelled && !state.closing)
    }
    pub(crate) fn cancel(&self, request: u64) {
        if let Ok(mut state) = self.state.lock()
            && state.request == request
        {
            state.cancelled = true;
            self.changed.notify_all();
        }
    }
    pub(crate) fn closing(&self) -> bool {
        self.state.lock().map_or(true, |state| state.closing)
    }
    pub(crate) fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closing = true;
            self.changed.notify_all();
        }
    }
    pub(crate) fn stopped(&self) -> bool {
        self.state
            .lock()
            .map_or(true, |state| state.credit.stopped())
    }
    pub(crate) fn cancelled(&self) -> bool {
        self.state.lock().map_or(true, |state| state.cancelled)
    }
}
fn check_request(request: u64, key: FlowKey) -> Result<(), ProtocolError> {
    if key.request == request {
        Ok(())
    } else {
        Err(ProtocolError::Stale)
    }
}
