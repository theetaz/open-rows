// SPDX-License-Identifier: Apache-2.0
//! Session states and the per-query event protocol (docs/04 §3, docs/06 §4).
use crate::CoreError;
use arrow_schema::SchemaRef;
use openrows_driver_api::NoticeLevel;
use openrows_types::{QueryId, ResultId, SessionId};
use std::time::Duration;

/// Lifecycle subset of docs/04 §3; transactions and reconnect arrive later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    /// The driver is connecting and authenticating.
    Connecting,
    /// Connected and ready for the next statement.
    Idle,
    /// Running a statement; later commands wait in the queue.
    Busy,
    /// Closed on request; terminal.
    Closed,
    /// Connection lost, connect failed or the driver panicked; terminal.
    Failed,
}

/// A session state transition, broadcast to every subscriber.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionEvent {
    /// Session that changed state.
    pub session: SessionId,
    /// The new state.
    pub state: SessionState,
    /// Why the session failed, for `Failed` transitions.
    pub error: Option<CoreError>,
}

/// Progress of one query, in order; exactly one terminal event ends the stream.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryEvent {
    /// The session actor started executing the statement.
    Started {
        /// The query that started.
        query: QueryId,
    },
    /// A result set opened; its batches go to the [`crate::BatchSink`].
    ResultBegin {
        /// Result identifier used with the sink.
        result: ResultId,
        /// Arrow schema with `openrows.*` field metadata.
        schema: SchemaRef,
    },
    /// Rows received so far; coalesced to at most 10 per second.
    Progress {
        /// Result receiving the rows.
        result: ResultId,
        /// Rows delivered so far.
        rows: u64,
        /// Arrow memory delivered so far.
        bytes: u64,
    },
    /// A result set is complete.
    ResultEnd {
        /// Result that ended.
        result: ResultId,
        /// Rows delivered to the sink.
        rows: u64,
        /// The sink stopped the stream before all rows arrived.
        truncated: bool,
    },
    /// A statement without a result set reported its row count.
    RowsAffected {
        /// Rows inserted, updated, deleted or copied.
        count: u64,
    },
    /// A server notice raised while the statement ran.
    Notice {
        /// Severity.
        level: NoticeLevel,
        /// Server message.
        message: String,
    },
    /// Terminal: the statement finished.
    Completed {
        /// Time from `Started` to completion.
        elapsed: Duration,
    },
    /// Terminal: the statement failed.
    Failed {
        /// What went wrong.
        error: CoreError,
    },
    /// Terminal: the statement was cancelled before or while running.
    Cancelled,
}

impl QueryEvent {
    /// True for `Completed`, `Failed` and `Cancelled`.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Failed { .. } | Self::Cancelled
        )
    }
}
