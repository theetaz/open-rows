// SPDX-License-Identifier: Apache-2.0
//! Prevent interleaved submissions during session-scoped explain options.
use super::{SessionManager, Submission};
use crate::{BatchSink, CoreError, ErrorKind, QueryHandle};
use openrows_sql::select::BoundSql;
use openrows_types::{QueryId, SessionId};
use std::sync::{Arc, PoisonError};

/// A reserved session; dropping the guard admits ordinary submissions again.
pub(crate) struct SessionSequence<'a> {
    manager: &'a SessionManager,
    session: SessionId,
    id: QueryId,
}

impl SessionManager {
    /// Reserve future submissions; already queued statements precede the sequence.
    pub(crate) fn sequence(&self, session: SessionId) -> Result<SessionSequence<'_>, CoreError> {
        let mut sequences = self
            .sequences
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if self.state(session).is_none() {
            return Err(CoreError::unknown_session(session));
        }
        if sequences.contains_key(&session) {
            return Err(CoreError::new(
                ErrorKind::QueueFull,
                "The session is busy explaining a statement",
            ));
        }
        let id = QueryId::new();
        sequences.insert(session, id);
        Ok(SessionSequence {
            manager: self,
            session,
            id,
        })
    }
}

impl SessionSequence<'_> {
    /// Submit one reserved statement to the same session actor.
    pub(crate) fn execute_into(
        &self,
        statement: BoundSql,
        sink: Arc<dyn BatchSink>,
    ) -> Result<QueryHandle, CoreError> {
        self.manager.submit(
            self.session,
            Submission {
                sink: Some(sink),
                sequence: Some(self.id),
                ..Submission::new(statement.sql, statement.params)
            },
        )
    }
}

impl Drop for SessionSequence<'_> {
    fn drop(&mut self) {
        self.manager
            .sequences
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.session);
    }
}
