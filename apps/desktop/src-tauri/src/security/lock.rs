// SPDX-License-Identifier: Apache-2.0
//! Shared lock state with monotonic, injectable time; reads never count as user input.
use crate::connections::{IpcError, IpcErrorKind};
use std::time::Duration;

/// State shared by every window and IPC entry point.
#[derive(Default)]
pub(crate) struct Lock {
    pub(crate) locked: bool,
    last_input: Duration,
    timeout: Option<Duration>,
    generation: u64,
}

impl Lock {
    /// Configure inactivity and apply startup locking only during installation.
    pub(crate) fn configure(&mut self, security: &serde_json::Value, startup: bool, now: Duration) {
        self.timeout = security
            .get("lockAfterInactivityMinutes")
            .and_then(serde_json::Value::as_u64)
            .filter(|minutes| [1, 5, 15, 30, 60].contains(minutes))
            .map(|minutes| Duration::from_secs(minutes * 60));
        if startup
            && security
                .get("lockOnStartup")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        {
            self.lock();
        }
        if startup {
            self.last_input = now;
        }
    }
    /// Lock and invalidate an in-flight unlock.
    pub(crate) fn lock(&mut self) {
        self.locked = true;
        self.generation += 1;
    }
    /// Snapshot the generation before native authentication.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
    /// Only the same lock generation can be unlocked by a successful authentication.
    pub(crate) fn unlock(&mut self, generation: u64, now: Duration) -> Result<(), IpcError> {
        if generation != self.generation {
            return Err(IpcError::new(
                IpcErrorKind::Locked,
                "OpenRows locked again during authentication",
            ));
        }
        self.locked = false;
        self.last_input = now;
        Ok(())
    }
    /// Genuine user input in any focused window resets the deadline.
    pub(crate) fn input(&mut self, now: Duration) {
        if !self.locked {
            self.last_input = now;
        }
    }
    /// Check the deadline even when no window is sending messages.
    pub(crate) fn tick(&mut self, now: Duration) {
        if !self.locked
            && self
                .timeout
                .is_some_and(|timeout| now.saturating_sub(self.last_input) >= timeout)
        {
            self.lock();
        }
    }
    /// Refuse every data command while locked; allow only lock-screen necessities.
    pub(crate) fn admit(&self, command: &str) -> Result<(), IpcError> {
        if !self.locked
            || matches!(
                command,
                "security_status"
                    | "security_unlock"
                    | "security_lock"
                    | "security_activity"
                    | "app_info"
                    | "settings_read"
                    | "ui_report"
                    | "menu_sync"
            )
        {
            return Ok(());
        }
        Err(IpcError::new(
            IpcErrorKind::Locked,
            "OpenRows is locked. Authenticate to access data.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_inactivity_and_every_data_command_are_guarded() -> Result<(), IpcError> {
        let mut lock = Lock::default();
        lock.configure(
            &serde_json::json!({"lockOnStartup":true,"lockAfterInactivityMinutes":1}),
            true,
            Duration::ZERO,
        );
        for command in [
            "query_execute",
            "table_page",
            "result_window",
            "table_edit",
            "connection_open",
            "history_search",
            "workspace_state_read",
            "saved_queries_list",
            "object_change",
            "table_alter",
            "dump_restore_start",
            "import_start",
        ] {
            assert_eq!(
                lock.admit(command).err().map(|e| e.kind),
                Some(IpcErrorKind::Locked)
            );
        }
        lock.unlock(lock.generation(), Duration::ZERO)?;
        assert!(lock.admit("table_page").is_ok());
        lock.input(Duration::from_secs(59));
        lock.tick(Duration::from_secs(60));
        assert!(!lock.locked);
        lock.tick(Duration::from_secs(119));
        assert!(lock.locked);
        let generation = lock.generation();
        lock.lock();
        assert!(lock.unlock(generation, Duration::from_secs(120)).is_err());
        Ok(())
    }
    #[test]
    fn every_registered_data_command_is_locked_then_admitted_after_unlock() -> Result<(), IpcError>
    {
        let mut lock = Lock::default();
        lock.lock();
        let mut checked = 0;
        for line in include_str!("../../build.rs").lines() {
            let Some(command) = line
                .trim()
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix(r#"","#))
            else {
                continue;
            };
            if matches!(
                command,
                "security_status"
                    | "security_unlock"
                    | "security_lock"
                    | "security_activity"
                    | "app_info"
                    | "settings_read"
                    | "ui_report"
                    | "menu_sync"
            ) {
                continue;
            }
            assert_eq!(
                lock.admit(command).err().map(|e| e.kind),
                Some(IpcErrorKind::Locked),
                "{command}"
            );
            checked += 1;
        }
        assert!(checked > 80);
        lock.unlock(lock.generation(), Duration::ZERO)?;
        assert!(lock.admit("query_execute").is_ok());
        assert!(lock.admit("result_window").is_ok());
        Ok(())
    }
}
