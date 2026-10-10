// SPDX-License-Identifier: Apache-2.0
//! Desktop-owned lazy launch policy; native engine registration remains host-only.
use crate::{
    CircuitBreaker, HostDriverId, HostFailure, HostSupervisor, LaunchOptions, MemoryBudget,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};

/// One app's installed helper, private endpoints and shared memory admission.
pub struct DriverHostRuntime {
    app_executable: PathBuf,
    endpoint_root: PathBuf,
    memory: MemoryBudget,
    circuits: Mutex<BTreeMap<(String, HostDriverId), CircuitBreaker>>,
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
        Ok(HostSupervisor::new(LaunchOptions {
            executable,
            driver,
            endpoint_root: self.endpoint_root.clone(),
            memory: self.memory.clone(),
            circuit,
        }))
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
