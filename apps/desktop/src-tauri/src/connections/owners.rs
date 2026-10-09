// SPDX-License-Identifier: Apache-2.0
//! Which saved connection and database each open user session belongs to.
//!
//! `PostgreSQL` cannot switch databases on a connection, so every opened
//! database of a connection has its own user session (plus a metadata session,
//! see [`super::metadata`]).
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use openrows_store::profile::ProfileId;
use openrows_types::SessionId;

/// The saved connection and database a user session runs on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Owner {
    pub(crate) profile: ProfileId,
    pub(crate) database: String,
}

/// Session → owner map shared by the session and query commands.
#[derive(Default)]
pub(crate) struct Owners {
    map: Mutex<HashMap<SessionId, Owner>>,
}

impl Owners {
    /// Remember that `session` was opened for `owner`.
    pub(crate) fn bind(&self, session: SessionId, owner: Owner) {
        self.lock().insert(session, owner);
    }

    /// Forget `session`, returning who owned it.
    pub(crate) fn unbind(&self, session: SessionId) -> Option<Owner> {
        self.lock().remove(&session)
    }

    /// The saved connection and database of an open session.
    pub(crate) fn owner(&self, session: SessionId) -> Option<Owner> {
        self.lock().get(&session).cloned()
    }

    /// Every open session of `profile`.
    pub(crate) fn sessions(&self, profile: ProfileId) -> Vec<SessionId> {
        self.lock()
            .iter()
            .filter(|(_, owner)| owner.profile == profile)
            .map(|(session, _)| *session)
            .collect()
    }

    /// Forget and return every session of `profile`.
    pub(crate) fn unbind_profile(&self, profile: ProfileId) -> Vec<SessionId> {
        let mut map = self.lock();
        let sessions: Vec<SessionId> = map
            .iter()
            .filter(|(_, owner)| owner.profile == profile)
            .map(|(session, _)| *session)
            .collect();
        for session in &sessions {
            map.remove(session);
        }
        sessions
    }

    /// Whether any session of `profile` still runs on `database`.
    pub(crate) fn in_use(&self, profile: ProfileId, database: &str) -> bool {
        self.lock()
            .values()
            .any(|owner| owner.profile == profile && owner.database == database)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<SessionId, Owner>> {
        self.map.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(profile: ProfileId, database: &str) -> Owner {
        Owner {
            profile,
            database: database.to_owned(),
        }
    }

    #[test]
    fn each_database_session_keeps_its_own_owner() {
        let owners = Owners::default();
        let profile = ProfileId::new();
        let (orders, reports) = (SessionId::new(), SessionId::new());
        owners.bind(orders, owner(profile, "orders"));
        owners.bind(reports, owner(profile, "reports"));
        assert_eq!(owners.owner(orders), Some(owner(profile, "orders")));
        assert_eq!(owners.owner(reports), Some(owner(profile, "reports")));
        assert_eq!(owners.unbind(orders), Some(owner(profile, "orders")));
        assert!(!owners.in_use(profile, "orders"));
        assert!(owners.in_use(profile, "reports"));
        assert_eq!(owners.owner(orders), None);
    }

    #[test]
    fn closing_a_connection_unbinds_only_its_sessions() {
        let owners = Owners::default();
        let (first, second) = (ProfileId::new(), ProfileId::new());
        let sessions = [SessionId::new(), SessionId::new()];
        owners.bind(sessions[0], owner(first, "orders"));
        owners.bind(sessions[1], owner(first, "reports"));
        let other = SessionId::new();
        owners.bind(other, owner(second, "orders"));
        let mut expected = sessions.to_vec();
        expected.sort();
        let mut listed = owners.sessions(first);
        listed.sort();
        assert_eq!(listed, expected);
        let mut closed = owners.unbind_profile(first);
        closed.sort();
        assert_eq!(closed, expected);
        assert!(!owners.in_use(first, "orders"));
        assert!(owners.in_use(second, "orders"));
        assert_eq!(owners.owner(other), Some(owner(second, "orders")));
    }
}
