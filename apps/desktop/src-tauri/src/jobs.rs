// SPDX-License-Identifier: Apache-2.0
//! What background jobs (exports, `.sql` file runs) share: the saved
//! connection and database of the webview session that started them. A job
//! opens a session of its own there, so it neither waits for nor blocks the
//! user's statements, and it keeps running when the tab closes.
use std::str::FromStr;

use openrows_core::safety::SafetyPolicy;
use openrows_sql::Dialect;
use openrows_types::SessionId;

use crate::connections::{Connections, IpcError, IpcErrorKind};
use crate::queries::commands::policy;

/// The saved connection and database a job runs on.
#[derive(Clone, Debug)]
pub(crate) struct JobScope {
    pub(crate) profile_id: String,
    pub(crate) database: String,
    /// The connection's safety switches and the engine's dialect.
    pub(crate) policy: SafetyPolicy,
}

impl JobScope {
    pub(crate) fn dialect(&self) -> Dialect {
        self.policy.dialect
    }

    /// Identifies the database in confirmation tokens.
    pub(crate) fn key(&self) -> String {
        format!("{}/{}", self.profile_id, self.database)
    }
}

/// The scope of an open webview session.
pub(crate) fn job_scope(connections: &Connections, session_id: &str) -> Result<JobScope, IpcError> {
    let session =
        SessionId::from_str(session_id).map_err(|_| IpcError::invalid("Invalid session id"))?;
    let owner = connections
        .owners
        .owner(session)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "The session is not open"))?;
    let profile = connections.profiles.get(owner.profile)?;
    Ok(JobScope {
        profile_id: owner.profile.to_string(),
        database: owner.database,
        policy: policy(&profile),
    })
}
