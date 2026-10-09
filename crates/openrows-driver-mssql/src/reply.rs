// SPDX-License-Identifier: Apache-2.0
//! Reading one request's reply: result sets, messages and row counts go to
//! a [`Rows`] consumer, environment changes into [`Env`]; the counts, the
//! first error, the login acknowledgement and an acknowledged attention end
//! up in [`Outcome`].
//!
//! An attention sent by the cancel handle is acknowledged by a `DONE` with
//! the attention bit, either ending this reply or, when the request had
//! already finished, as a message of its own that is read and dropped
//! here, so the next request starts on a clean stream.

use std::sync::Arc;

use openrows_driver_api::DriverError;

use crate::tds::collation::Collation;
use crate::tds::cursor::{Cursor, Short};
use crate::tds::envchange::EnvChange;
use crate::tds::token::{Column, Done, LoginAck, Message, RowValues, Token, token};
use crate::tds::wire::Wire;

/// A consumer of result sets.
pub(crate) trait Rows: Send {
    /// A result set starts.
    ///
    /// # Errors
    /// The consumer refuses the columns.
    fn begin(&mut self, columns: &Arc<[Column]>) -> Result<(), DriverError>;

    /// One row; `false` stops delivery (the rest of the reply is read and
    /// dropped).
    ///
    /// # Errors
    /// A value cannot be decoded.
    fn row(&mut self, values: RowValues<'_>) -> Result<bool, DriverError>;

    /// The result set ended.
    ///
    /// # Errors
    /// The consumer cannot finish its batch.
    fn end(&mut self) -> Result<(), DriverError>;

    /// A message other than the reply's first error (`PRINT`, warnings,
    /// later errors).
    fn notice(&mut self, _message: &Message) {}

    /// A statement reported its row count.
    fn counted(&mut self, _count: Count) {}
}

/// Session state the server reports through `ENVCHANGE`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Env {
    /// Descriptor of the open transaction; 0 outside one.
    pub(crate) transaction: u64,
    pub(crate) database: String,
    /// The database's default collation, sent with text parameters.
    pub(crate) collation: Collation,
}

/// The row count one statement reported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Count {
    pub(crate) rows: u64,
    /// The statement returned a result set (its rows were counted).
    pub(crate) result: bool,
}

/// How a reply ended.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    /// Row counts of the statements that had one, in order.
    pub(crate) counts: Vec<Count>,
    /// The first error the server reported.
    pub(crate) error: Option<Message>,
    /// The server acknowledged an attention within this reply.
    pub(crate) cancelled: bool,
    /// A new packet size the server chose.
    pub(crate) packet_size: Option<usize>,
    /// The login acknowledgement, in the reply to `LOGIN7`.
    pub(crate) login: Option<LoginAck>,
}

/// The statement being read.
#[derive(Clone, Copy, Debug, Default)]
struct Statement {
    /// A result set is open in the consumer.
    open: bool,
    /// A result set began since the last `DONE`.
    had_result: bool,
}

struct Reader<'a> {
    env: &'a mut Env,
    rows: &'a mut dyn Rows,
    columns: Arc<[Column]>,
    statement: Statement,
    delivering: bool,
    /// The consumer's first error; the reply is still read to its end.
    failure: Option<DriverError>,
    /// The final `DONE` reported an error.
    aborted: bool,
    outcome: Outcome,
}

/// Read the reply to the request just sent.
///
/// # Errors
/// `Network` and `Protocol` failures, consumer errors, and a routing
/// redirect (`Unsupported`). Server errors are in [`Outcome::error`].
pub(crate) async fn read_reply(
    wire: &mut Wire,
    env: &mut Env,
    rows: &mut dyn Rows,
) -> Result<Outcome, DriverError> {
    let mut reader = Reader {
        env: &mut *env,
        rows,
        columns: Arc::from(Vec::new()),
        statement: Statement::default(),
        delivering: true,
        failure: None,
        aborted: false,
        outcome: Outcome::default(),
    };
    let finished = reader.read(wire).await;
    let (mut outcome, failure) = (reader.outcome, reader.failure);
    let aborted = reader.aborted && outcome.error.is_none();
    let attention = wire.finish_request().await;
    finished?;
    if attention && !outcome.cancelled {
        drain_attention(wire, env).await?;
        // The statement ended in error without a message: the attention
        // stopped it before its acknowledgement was sent on its own.
        outcome.cancelled = aborted;
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(outcome),
    }
}

impl Reader<'_> {
    async fn read(&mut self, wire: &mut Wire) -> Result<(), DriverError> {
        loop {
            let parsed = {
                let mut cursor = Cursor::new(wire.inbound.bytes());
                token(&mut cursor, &self.columns).map(|token| (token, cursor.position()))
            };
            let (token, used) = match parsed {
                Ok(parsed) => parsed,
                Err(Short::Incomplete) => {
                    wire.inbound.fill().await?;
                    continue;
                }
                Err(Short::Invalid(error)) => return Err(error),
            };
            let last = self.handle(token);
            wire.inbound.consume(used);
            if last {
                return Ok(());
            }
        }
    }

    /// Handle one token; true for the reply's final `DONE`.
    fn handle(&mut self, token: Token<'_>) -> bool {
        match token {
            Token::Metadata(columns) => self.metadata(columns),
            Token::Row(values) if self.delivering => {
                let delivered = self.rows.row(values);
                self.delivering = self.consumed(delivered).unwrap_or(false);
            }
            Token::Done(done) => return self.done(done),
            Token::Message(message) => self.message(message),
            Token::EnvChange(change) => {
                let applied = self.env_change(change);
                self.consumed(applied);
            }
            Token::LoginAck(ack) => self.outcome.login = Some(ack),
            Token::Row(_) | Token::Skipped => {}
        }
        false
    }

    /// Keep the first failure and stop delivering; the reply is still read.
    fn consumed<T>(&mut self, result: Result<T, DriverError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.delivering = false;
                self.statement.open = false;
                self.failure.get_or_insert(error);
                None
            }
        }
    }

    fn metadata(&mut self, columns: Option<Arc<[Column]>>) {
        self.close();
        if let Some(columns) = columns {
            self.statement.had_result = true;
            if self.delivering {
                let begun = self.rows.begin(&columns);
                self.statement.open = self.consumed(begun).is_some();
            }
            self.columns = columns;
        }
    }

    fn close(&mut self) {
        if std::mem::take(&mut self.statement.open) {
            let ended = self.rows.end();
            self.consumed(ended);
        }
    }

    fn done(&mut self, done: Done) -> bool {
        self.close();
        if let Some(rows) = done.count() {
            let count = Count {
                rows,
                result: std::mem::take(&mut self.statement.had_result),
            };
            self.rows.counted(count);
            self.outcome.counts.push(count);
        }
        self.outcome.cancelled |= done.attention();
        let last = !done.more() && !done.in_proc;
        self.aborted = last && done.error();
        last
    }

    fn message(&mut self, message: Message) {
        if message.error && self.outcome.error.is_none() {
            self.outcome.error = Some(message);
        } else {
            self.rows.notice(&message);
        }
    }

    fn env_change(&mut self, change: EnvChange) -> Result<(), DriverError> {
        match change {
            EnvChange::Database(name) => self.env.database = name,
            EnvChange::PacketSize(size) => self.outcome.packet_size = Some(size),
            EnvChange::Collation(bytes) => self.env.collation = Collation::from_bytes(&bytes),
            EnvChange::BeginTransaction(descriptor) => self.env.transaction = descriptor,
            EnvChange::EndTransaction => self.env.transaction = 0,
            EnvChange::Routing { host, port } => {
                return Err(DriverError::Unsupported(format!(
                    "the server redirects connections to {host}:{port} (Azure SQL \"Redirect\" \
                     connection policy); connect to that host directly or use the \"Proxy\" policy"
                )));
            }
            EnvChange::Other => {}
        }
        Ok(())
    }
}

/// Read and drop messages until the attention acknowledgement, keeping
/// their environment changes (a cancel may end the transaction).
async fn drain_attention(wire: &mut Wire, env: &mut Env) -> Result<(), DriverError> {
    loop {
        wire.inbound.next_message();
        let mut reader = Reader {
            env: &mut *env,
            rows: &mut Ignore,
            columns: Arc::from(Vec::new()),
            statement: Statement::default(),
            delivering: false,
            failure: None,
            aborted: false,
            outcome: Outcome::default(),
        };
        reader.read(wire).await?;
        if reader.outcome.cancelled {
            return Ok(());
        }
    }
}

/// Drops every result.
pub(crate) struct Ignore;

impl Rows for Ignore {
    fn begin(&mut self, _columns: &Arc<[Column]>) -> Result<(), DriverError> {
        Ok(())
    }

    fn row(&mut self, _values: RowValues<'_>) -> Result<bool, DriverError> {
        Ok(false)
    }

    fn end(&mut self) -> Result<(), DriverError> {
        Ok(())
    }
}
