// SPDX-License-Identifier: Apache-2.0
//! Investigation/advice read the same bounded, redacted monitor generation.
use super::{Monitor, expire};
use crate::{
    Investigation, MonitorError,
    advisor::{AdvisorInput, PlanEvidence, Suggestion, advise},
};
use std::sync::{PoisonError, atomic::Ordering};

/// A per-collector diagnostic reservation released on every completion/cancellation path.
pub struct DiagnosticGuard<'a>(&'a Monitor);
impl Drop for DiagnosticGuard<'_> {
    fn drop(&mut self) {
        self.0.diagnostic.store(false, Ordering::SeqCst);
    }
}
impl Monitor {
    /// One explicit diagnostic per collector; never allow overlapping measured execution.
    /// # Errors
    /// Stopped or already busy.
    pub fn begin_diagnostic(&self) -> Result<DiagnosticGuard<'_>, MonitorError> {
        if *self.stopped.borrow() {
            return Err(MonitorError::Stopped);
        }
        self.diagnostic
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| MonitorError::Limit)?;
        Ok(DiagnosticGuard(self))
    }
    /// Issue/consume one bounded native challenge for this immutable diagnostic binding.
    /// # Errors
    /// Stopped generation or an oversized binding/token.
    pub fn confirm_diagnostic(
        &self,
        binding: &str,
        supplied: Option<&str>,
    ) -> Result<Option<String>, MonitorError> {
        let shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if shared.stopped {
            return Err(MonitorError::Stopped);
        }
        if binding.len() > 128 || supplied.is_some_and(|token| token.len() > 256) {
            return Err(MonitorError::Limit);
        }
        let mut pending = self.consent.lock().unwrap_or_else(PoisonError::into_inner);
        if pending.as_ref().is_some_and(|(expected, challenge)| {
            expected == binding && supplied == Some(challenge.as_str())
        }) {
            *pending = None;
            return Ok(None);
        }
        let sequence = self.consent_sequence.fetch_add(1, Ordering::SeqCst);
        let challenge = format!("{}:{sequence}:{binding}", self.generation);
        *pending = Some((binding.into(), challenge.clone()));
        Ok(Some(challenge))
    }
    /// Diagnostic requests observe stop/lock/profile invalidation immediately.
    #[must_use]
    pub fn stopped_signal(&self) -> tokio::sync::watch::Receiver<bool> {
        self.stopped.subscribe()
    }
    /// Weighted live-query detail with reset-aware trends and documented regressions.
    /// # Errors
    /// Stopped generation.
    pub fn investigate(&self, id: &str) -> Result<Investigation, MonitorError> {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if shared.stopped {
            return Err(MonitorError::Stopped);
        }
        expire(&mut shared);
        Ok(shared.store.investigate(id))
    }
    /// Permitted evidence for native diagnostics; no raw SQL is present.
    /// # Errors
    /// Stopped generation.
    pub fn advisor_input(&self, now: f64) -> Result<AdvisorInput, MonitorError> {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if shared.stopped {
            return Err(MonitorError::Stopped);
        }
        expire(&mut shared);
        Ok(shared.store.advisor_input(now))
    }
    /// Evaluate bounded versioned proposals without executing anything.
    /// # Errors
    /// Stopped generation.
    pub fn advice(&self, now: f64) -> Result<Vec<Suggestion>, MonitorError> {
        Ok(advise(&self.advisor_input(now)?))
    }
    /// Retain only the safe projection; late replies cannot revive a stopped generation.
    /// # Errors
    /// Stopped generation.
    pub fn record_plan(&self, plan: PlanEvidence) -> Result<(), MonitorError> {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        if shared.stopped {
            return Err(MonitorError::Stopped);
        }
        shared.store.record_plan(plan);
        expire(&mut shared);
        Ok(())
    }
}
