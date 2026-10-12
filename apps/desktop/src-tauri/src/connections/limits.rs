// SPDX-License-Identifier: Apache-2.0
//! Session limits of each connection (M1-01, docs/04 §3.2 and §6).
//!
//! Statement timeout: the profile's value, else the app-wide default
//! (`general.statementTimeoutSeconds` in settings.json, off by default). The
//! core cancels a statement past it through the normal cancel path; server
//! sessions also set explicit profile server-side limits at session start and after every
//! reconnect (`PostgreSQL` `statement_timeout`, `MySQL` `max_execution_time`
//! for `SELECT`, `MariaDB` `max_statement_time`).
//!
//! Idle timeout: every [`SWEEP`] the user sessions of a profile with an idle
//! timeout that ran nothing for that long are suspended (their connection
//! closes; the next statement reopens it) and its idle metadata connections
//! close. A session holding an open transaction (the core checks) or a
//! database with staged table edits (the change-set journal) is kept.
use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use openrows_store::LocalStore;
use openrows_store::profile::{ConnectionProfile, Driver, ProfileId};
use openrows_types::SessionId;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::Connections;

/// How often idle sessions are looked for.
const SWEEP: Duration = Duration::from_secs(30);
/// Longest app-wide default accepted from settings.json (one day).
const MAX_DEFAULT_SECONDS: u64 = 86_400;

/// The app-wide statement timeout, read from settings.json.
#[derive(Default)]
pub(crate) struct DefaultLimits {
    statement_timeout_seconds: AtomicU32,
}

impl DefaultLimits {
    /// Apply `general.statementTimeoutSeconds`; missing or invalid is `0` (none).
    pub(crate) fn apply(&self, settings: &Value) {
        let seconds = settings
            .pointer("/general/statementTimeoutSeconds")
            .and_then(Value::as_u64)
            .map_or(0, |seconds| seconds.min(MAX_DEFAULT_SECONDS));
        let seconds = u32::try_from(seconds).unwrap_or(0);
        self.statement_timeout_seconds
            .store(seconds, Ordering::Relaxed);
    }

    /// Apply the current saved default at the IPC/core boundary for a new run.
    pub(crate) fn apply_to_session(
        &self,
        manager: &openrows_core::SessionManager,
        session: SessionId,
        profile: &ConnectionProfile,
    ) -> Result<(), openrows_core::CoreError> {
        manager.set_statement_timeout(session, self.statement_timeout(profile))
    }

    /// The statement timeout for sessions of `profile`.
    pub(crate) fn statement_timeout(&self, profile: &ConnectionProfile) -> Option<Duration> {
        let default = self.statement_timeout_seconds.load(Ordering::Relaxed);
        profile.limits.statement_timeout(default)
    }
}

/// Statements that set the server-side statement timeout at session start.
pub(crate) fn setup_sql(driver: Driver, timeout: Option<Duration>) -> Vec<String> {
    let Some(timeout) = timeout else {
        return Vec::new();
    };
    let millis = timeout.as_millis();
    match driver {
        Driver::Postgres => vec![format!("SET statement_timeout = {millis}")],
        Driver::Mysql => vec![format!("SET SESSION max_execution_time = {millis}")],
        Driver::Mariadb => vec![format!(
            "SET SESSION max_statement_time = {}",
            timeout.as_secs_f64()
        )],
        // SQL Server has no server-side statement timeout; the session's
        // timer cancels with the attention signal instead.
        Driver::Sqlserver
        | Driver::Clickhouse
        | Driver::Sqlite
        | Driver::Mongodb
        | Driver::Libsql
        | Driver::LibsqlFile
        | Driver::Duckdb => Vec::new(),
    }
}

/// Look for idle sessions every [`SWEEP`] for the app's lifetime.
pub(crate) fn watch_idle(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(SWEEP);
        loop {
            ticks.tick().await;
            sweep(&app.state::<Connections>(), &app.state::<LocalStore>()).await;
        }
    });
}

/// Suspend idle user sessions and close idle metadata connections.
async fn sweep(state: &Connections, store: &LocalStore) {
    let staged = staged_databases(store);
    for (session, owner) in state.owners.all() {
        let Some(idle) = idle_timeout(state, owner.profile) else {
            continue;
        };
        if staged.contains(&(owner.profile.to_string(), owner.database.clone())) {
            continue;
        }
        suspend(state, session, idle);
    }
    for profile in state.metadata.profiles() {
        let id = super::profiles::profile_id(&profile).ok();
        if let Some(idle) = id.and_then(|id| idle_timeout(state, id)) {
            state.metadata.close_idle(&profile, idle).await;
        }
    }
}

fn suspend(state: &Connections, session: SessionId, idle: Duration) {
    // A session that closed meanwhile has nothing left to suspend.
    let _ = state.sessions.suspend_idle(session, idle);
}

fn idle_timeout(state: &Connections, profile: ProfileId) -> Option<Duration> {
    state.profiles.get(profile).ok()?.limits.idle_timeout()
}

/// Connection and database of every table with journaled staged edits; the
/// journal id is `profile \n database \n schema \n table`.
fn staged_databases(store: &LocalStore) -> HashSet<(String, String)> {
    // Without the journal nothing is known to be staged; edits never depend
    // on the user session (they commit on their own connection).
    let journaled = store.change_sets().unwrap_or_default();
    journaled
        .iter()
        .filter_map(|record| {
            let mut parts = record.id.split('\n');
            Some((parts.next()?.to_owned(), parts.next()?.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_side_timeouts_per_engine() {
        let limit = Some(Duration::from_millis(1500));
        assert_eq!(
            setup_sql(Driver::Postgres, limit),
            ["SET statement_timeout = 1500"]
        );
        assert_eq!(
            setup_sql(Driver::Mysql, limit),
            ["SET SESSION max_execution_time = 1500"]
        );
        assert_eq!(
            setup_sql(Driver::Mariadb, limit),
            ["SET SESSION max_statement_time = 1.5"]
        );
        assert!(setup_sql(Driver::Sqlite, limit).is_empty());
        assert!(setup_sql(Driver::Sqlserver, limit).is_empty());
        assert!(setup_sql(Driver::Postgres, None).is_empty());
    }

    #[test]
    fn the_default_comes_from_settings_and_the_profile_wins() -> Result<(), serde_json::Error> {
        let defaults = DefaultLimits::default();
        let mut profile = ConnectionProfile::new_postgres(
            "p",
            openrows_store::profile::ServerParams::default(),
            openrows_store::profile::EnvTag::new(openrows_store::profile::EnvKind::Local),
        );
        assert_eq!(defaults.statement_timeout(&profile), None);
        defaults.apply(&serde_json::from_str(
            r#"{"general":{"statementTimeoutSeconds":30}}"#,
        )?);
        assert_eq!(
            defaults.statement_timeout(&profile),
            Some(Duration::from_secs(30))
        );
        profile.limits.statement_timeout_seconds = Some(0);
        assert_eq!(defaults.statement_timeout(&profile), None);
        defaults.apply(&serde_json::from_str(
            r#"{"general":{"statementTimeoutSeconds":"x"}}"#,
        )?);
        profile.limits.statement_timeout_seconds = None;
        assert_eq!(defaults.statement_timeout(&profile), None);
        Ok(())
    }
}

#[cfg(test)]
mod run_tests {
    use super::*;
    use openrows_core::{QueryEvent, SessionManager};
    use std::sync::Arc;

    #[tokio::test]
    async fn a_saved_timeout_applies_to_new_runs_on_an_already_open_core_session()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let database = dir.path().join("timeout.db");
        let manager = SessionManager::new(Arc::new(openrows_results::ResultStore::default()));
        let session = manager
            .open(async move {
                openrows_driver_sqlite::open(&database, false, true)
                    .map(|session| Box::new(session) as Box<dyn openrows_driver_api::Session>)
            })
            .await?;
        let profile = ConnectionProfile::new_file(
            "Local",
            openrows_store::profile::FileParams::memory(),
            openrows_store::profile::EnvTag::new(openrows_store::profile::EnvKind::Local),
        );
        let defaults = DefaultLimits::default();
        defaults.apply(&serde_json::json!({"general": {"statementTimeoutSeconds": 1}}));
        defaults.apply_to_session(&manager, session, &profile)?;
        let mut query = manager.execute(
            session,
            "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n) SELECT sum(x) FROM n",
        )?;
        let mut timed_out = false;
        while let Some(event) = query.next_event().await {
            timed_out |= matches!(event, QueryEvent::Failed { error } if error.kind == openrows_core::ErrorKind::Timeout);
        }
        assert!(timed_out);
        defaults.apply(&serde_json::json!({"general": {"statementTimeoutSeconds": 0}}));
        defaults.apply_to_session(&manager, session, &profile)?;
        let mut next = manager.execute(session, "SELECT 1")?;
        let mut completed = false;
        while let Some(event) = next.next_event().await {
            completed |= matches!(event, QueryEvent::Completed { .. });
        }
        assert!(completed);
        manager.close(session).await?;
        Ok(())
    }
}
