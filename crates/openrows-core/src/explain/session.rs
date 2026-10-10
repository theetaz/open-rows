// SPDX-License-Identifier: Apache-2.0
//! A SQL Server sequence holds its session until option reset and rollback.
use crate::manager::SessionSequence;
use crate::{BatchSink, CoreError, QueryHandle, SessionManager};
use openrows_sql::select::BoundSql;
use openrows_types::SessionId;
use std::sync::Arc;

/// Executes explain statements, optionally reserving the session.
pub(super) struct ExplainSession<'a> {
    manager: &'a SessionManager,
    session: SessionId,
    sequence: Option<SessionSequence<'a>>,
}

impl<'a> ExplainSession<'a> {
    /// Reserve SQL Server options without changing other engines.
    pub(super) fn new(
        manager: &'a SessionManager,
        session: SessionId,
        reserve: bool,
    ) -> Result<Self, CoreError> {
        let sequence = if reserve {
            Some(manager.sequence(session)?)
        } else {
            None
        };
        Ok(Self {
            manager,
            session,
            sequence,
        })
    }

    /// Read transaction state after the queue-draining barrier.
    pub(super) fn in_transaction(&self) -> bool {
        self.manager.in_transaction(self.session).unwrap_or(false)
    }

    /// Collect one statement through the reservation when present.
    pub(super) fn execute_into(
        &self,
        statement: BoundSql,
        sink: Arc<dyn BatchSink>,
    ) -> Result<QueryHandle, CoreError> {
        match &self.sequence {
            Some(sequence) => sequence.execute_into(statement, sink),
            None => self.manager.execute_into(self.session, statement, sink),
        }
    }
}
