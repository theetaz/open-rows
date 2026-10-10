// SPDX-License-Identifier: Apache-2.0
//! Desktop-owned lazy launch policy; native engine registration remains host-only.
use crate::{
    CircuitBreaker, HostDriverId, HostFailure, HostSupervisor, LaunchOptions, MemoryBudget,
    PrewarmOutcome, PrewarmReport, prewarm::Prewarm,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};

/// One app's installed helper, private endpoints and shared memory admission.
pub struct DriverHostRuntime {
    app_executable: PathBuf,
    endpoint_root: PathBuf,
    memory: MemoryBudget,
    circuits: Mutex<BTreeMap<(String, HostDriverId), CircuitBreaker>>,
    prewarm: Prewarm,
    #[cfg(debug_assertions)]
    debug_helper: Option<PathBuf>,
}
impl DriverHostRuntime {
    /// Preserve the installed layout without spawning or opening a database.
    #[must_use]
    pub fn new(app_executable: PathBuf, endpoint_root: PathBuf) -> Self {
        Self {
            app_executable,
            endpoint_root,
            memory: MemoryBudget::default(),
            circuits: Mutex::default(),
            prewarm: Prewarm::default(),
            #[cfg(debug_assertions)]
            debug_helper: None,
        }
    }
    /// Explicit absolute helper override; this API is absent from release builds.
    /// # Errors
    /// Relative helper paths are refused.
    #[cfg(debug_assertions)]
    pub fn with_debug_helper(mut self, helper: PathBuf) -> Result<Self, HostFailure> {
        if !helper.is_absolute() {
            return Err(HostFailure::Spawn);
        }
        self.debug_helper = Some(helper);
        Ok(self)
    }
    /// Resolve afresh and reuse the same app-owned profile/driver crash circuit.
    /// # Errors
    /// Typed helper validation or private endpoint-directory failure; never a fallback.
    pub fn launcher(
        &self,
        driver: HostDriverId,
        profile: &str,
    ) -> Result<HostSupervisor, openrows_driver_api::DriverError> {
        let executable = self.executable()?;
        private_root(&self.endpoint_root)?;
        let circuit = self
            .circuits
            .lock()
            .map_err(|_| HostFailure::Spawn)?
            .entry((profile.into(), driver))
            .or_default()
            .clone();
        let supervisor = HostSupervisor::new(LaunchOptions {
            executable,
            driver,
            endpoint_root: self.endpoint_root.clone(),
            memory: self.memory.clone(),
            circuit,
        });
        Ok(if driver == HostDriverId::Duckdb {
            supervisor.with_prewarm(self.prewarm.clone())
        } else {
            supervisor
        })
    }

    /// Attempt database-free installed Probe once, after the window has painted.
    /// Errors become diagnostics only; `DuckDB` connects await an in-flight attempt.
    pub async fn prewarm(&self) -> Option<PrewarmReport> {
        let attempt = self.prewarm.start()?;
        let executable = match self.executable().and_then(|executable| {
            private_root(&self.endpoint_root)?;
            Ok(executable)
        }) {
            Ok(executable) => executable,
            Err(error) => return Some(attempt.finish(PrewarmOutcome::Skipped(error), None)),
        };
        // Share memory admission, but never mutate any profile's crash circuit.
        let probe = HostSupervisor::new(LaunchOptions {
            executable,
            driver: HostDriverId::Probe,
            endpoint_root: self.endpoint_root.clone(),
            memory: self.memory.clone(),
            circuit: CircuitBreaker::default(),
        });
        let started = std::time::Instant::now();
        let (outcome, handshake) = match probe.spawn().await {
            Ok(host) => {
                let handshake = Some(started.elapsed());
                let outcome = match host.close().await {
                    Ok(()) => PrewarmOutcome::Ready,
                    Err(error) => PrewarmOutcome::Failed(error),
                };
                (outcome, handshake)
            }
            Err(error) => (PrewarmOutcome::Failed(error), None),
        };
        Some(attempt.finish(outcome, handshake))
    }

    /// Last prewarm outcome and timing; absent before completion.
    #[must_use]
    pub fn prewarm_report(&self) -> Option<PrewarmReport> {
        self.prewarm.report()
    }
    fn executable(&self) -> Result<PathBuf, HostFailure> {
        #[cfg(debug_assertions)]
        if let Some(helper) = &self.debug_helper {
            crate::resolve::validate(helper)?;
            return Ok(helper.clone());
        }
        crate::resolve_installed(&self.app_executable)
    }
}
fn private_root(root: &std::path::Path) -> Result<(), HostFailure> {
    if !root.is_absolute() {
        return Err(HostFailure::Spawn);
    }
    std::fs::create_dir_all(root).map_err(|_| HostFailure::Spawn)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| HostFailure::Spawn)?;
    }
    Ok(())
}
