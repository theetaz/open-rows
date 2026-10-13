// SPDX-License-Identifier: Apache-2.0
//! Extended-protocol execution with per-column result formats.
mod cancellation;
use crate::cancel::PgCancel;
use crate::connect::establish;
use crate::decode::ResultDecoder;
use crate::info::ServerInfo;
use crate::options::ConnectOptions;
use crate::simple;
use crate::startup::PgConfig;
use crate::stream;
use crate::trace::ConnectTrace;
use crate::types::{self, ColumnDesc};
use crate::wire::{Wire, server_error, unexpected};
use fallible_iterator::FallibleIterator;
use openrows_driver_api::{
    BatchPolicy, BoxFuture, CancelHandle, DriverError, ExecSummary, ResultSink, ServerSummary,
    Session,
};
use postgres_protocol::IsNull;
use postgres_protocol::message::backend::{Message, RowDescriptionBody};
use postgres_protocol::message::frontend;
use std::{collections::HashMap, fmt::Write, iter, sync::Arc};

/// One `PostgreSQL` connection, owned by a single session actor (ADR-0007).
pub struct PgSession {
    pub(crate) monitor_config: Option<crate::monitor::Config>,
    pub(crate) wire: Wire,
    type_names: HashMap<u32, String>,
    policy: BatchPolicy,
    cancel: Option<Arc<PgCancel>>,
    info: ServerInfo,
    summary: ServerSummary,
}

impl PgSession {
    /// Connect with default [`ConnectOptions`]: `sslmode=prefer` (TLS without
    /// certificate checks when the server offers it), docs/14 §4 timeouts, no
    /// startup commands.
    ///
    /// # Errors
    /// As [`PgSession::connect_with`].
    pub async fn connect(config: &PgConfig) -> Result<Self, DriverError> {
        Self::connect_with(config, &ConnectOptions::default()).await
    }

    /// Connect, negotiate TLS, authenticate, read [`ServerInfo`] and run the
    /// startup commands.
    ///
    /// # Errors
    /// `Tls` for certificate files, verification and handshake failures or a
    /// server without TLS under a TLS-only `sslmode`; `Network` for transport
    /// failures and stage timeouts (`io::ErrorKind::TimedOut`, message names
    /// the stage); `Auth`; `Server` when the server refuses the session or a
    /// startup command fails (message prefixed `startup command <n>:`);
    /// `Protocol`.
    pub async fn connect_with(
        config: &PgConfig,
        options: &ConnectOptions,
    ) -> Result<Self, DriverError> {
        Self::connect_traced(config, options).await.0
    }

    /// As [`PgSession::connect_with`], also reporting which stages ran, how
    /// long each took, which one failed and the TLS outcome (Test connection).
    pub async fn connect_traced(
        config: &PgConfig,
        options: &ConnectOptions,
    ) -> (Result<Self, DriverError>, ConnectTrace) {
        let mut trace = ConnectTrace::default();
        let session = Self::connect_recording(config, options, &mut trace).await;
        (session, trace)
    }

    /// As [`PgSession::connect_with`], recording the stages in `trace`.
    ///
    /// # Errors
    /// As [`PgSession::connect_with`].
    pub async fn connect_recording(
        config: &PgConfig,
        options: &ConnectOptions,
        trace: &mut ConnectTrace,
    ) -> Result<Self, DriverError> {
        let established = establish(config, options, trace).await?;
        Ok(Self {
            monitor_config: None,
            wire: established.wire,
            type_names: HashMap::new(),
            policy: BatchPolicy::default(),
            cancel: established.cancel.map(Arc::new),
            summary: established.info.summary(),
            info: established.info,
        })
    }

    /// The engine-neutral server summary (see [`ServerInfo::summary`]).
    pub(crate) fn summary(&self) -> &ServerSummary {
        &self.summary
    }

    /// Product, version, current database and user, flavour and TLS state.
    #[must_use]
    pub fn server_info(&self) -> &ServerInfo {
        &self.info
    }

    /// Run internal SQL through the simple-query protocol and collect text rows.
    pub(crate) async fn text_query(
        &mut self,
        sql: &str,
    ) -> Result<Vec<simple::TextRow>, DriverError> {
        let statement = self.begin_statement().await?;
        let result = simple::query(&mut self.wire, sql).await;
        self.finish_statement(statement, result).await
    }

    async fn run(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        if params.iter().flatten().any(|value| value.contains('\0')) {
            return Err(DriverError::Unsupported(
                "parameter values containing NUL characters".to_owned(),
            ));
        }
        let columns = self.describe(sql, params.len()).await?;
        let reparse = self
            .resolve_type_names(columns.as_deref().unwrap_or_default())
            .await?;
        let decoder = columns.map(|c| ResultDecoder::new(&c, &self.type_names, self.policy));
        let formats = decoder
            .as_ref()
            .map_or(&[][..], ResultDecoder::result_formats);
        send_execute(self.wire.out(), Statement { sql, params }, reparse, formats)?;
        self.wire.flush().await?;
        stream::collect(&mut self.wire, decoder, sink).await
    }

    /// Parse + Describe the unnamed statement; `None` when it returns no rows.
    /// Parameter types are left to the server; it must expect `params` values.
    async fn describe(
        &mut self,
        sql: &str,
        params: usize,
    ) -> Result<Option<Vec<ColumnDesc>>, DriverError> {
        let out = self.wire.out();
        frontend::parse("", sql, iter::empty(), out)?;
        frontend::describe(b'S', "", out)?;
        frontend::sync(out);
        self.wire.flush().await?;
        let (mut columns, mut failure) = (None, None);
        loop {
            match self.wire.next().await? {
                Message::ParameterDescription(body) => {
                    let expected = body.parameters().count()?;
                    failure = failure.or_else(|| parameter_mismatch(expected, params));
                }
                Message::RowDescription(body) => columns = Some(column_descs(&body)?),
                Message::ErrorResponse(body) => failure = Some(server_error(&body)),
                Message::ReadyForQuery(_) => return failure.map_or(Ok(columns), Err),
                Message::ParseComplete
                | Message::NoData
                | Message::NoticeResponse(_)
                | Message::ParameterStatus(_) => {}
                _ => return Err(unexpected("describe")),
            }
        }
    }

    /// Look up names of non-built-in types once per session. A simple query
    /// discards the unnamed statement, so returns `true` when it must be re-parsed.
    async fn resolve_type_names(&mut self, columns: &[ColumnDesc]) -> Result<bool, DriverError> {
        let mut oids = String::new();
        for column in columns
            .iter()
            .filter(|c| types::needs_name(c, &self.type_names))
        {
            let separator = if oids.is_empty() { "" } else { "," };
            let _ = write!(oids, "{separator}{}", column.type_oid);
        }
        if oids.is_empty() {
            return Ok(false);
        }
        let sql = format!(
            "SELECT oid, format_type(oid, NULL) FROM pg_catalog.pg_type WHERE oid IN ({oids})"
        );
        for row in simple::query(&mut self.wire, &sql).await? {
            let invalid = || DriverError::Protocol("invalid type lookup row".to_owned());
            let [Some(oid), Some(name)] =
                <[Option<String>; 2]>::try_from(row).map_err(|_| invalid())?
            else {
                return Err(invalid());
            };
            self.type_names
                .insert(oid.parse().map_err(|_| invalid())?, name);
        }
        Ok(true)
    }
}

fn column_descs(body: &RowDescriptionBody) -> Result<Vec<ColumnDesc>, DriverError> {
    let descs = body
        .fields()
        .map(|field| {
            Ok(ColumnDesc {
                name: field.name().to_owned(),
                type_oid: field.type_oid(),
                type_modifier: field.type_modifier(),
            })
        })
        .collect()?;
    Ok(descs)
}

/// Statement text with its text-format parameter values.
#[derive(Clone, Copy)]
struct Statement<'a> {
    sql: &'a str,
    params: &'a [Option<String>],
}

/// `None` when the counts agree. Editor SQL has no parameters, so a `$1`
/// there is reported as unsupported.
fn parameter_mismatch(expected: usize, given: usize) -> Option<DriverError> {
    match (expected, given) {
        _ if expected == given => None,
        (_, 0) => Some(DriverError::Unsupported("bind parameters".to_owned())),
        _ => Some(DriverError::Protocol(format!(
            "the statement expects {expected} parameters but {given} were bound"
        ))),
    }
}

/// Bind (all parameters in text format, so the server applies each inferred
/// type's input function), Execute and Sync; Parse again when `reparse`.
fn send_execute(
    out: &mut bytes::BytesMut,
    statement: Statement<'_>,
    reparse: bool,
    formats: &[i16],
) -> Result<(), DriverError> {
    if reparse {
        frontend::parse("", statement.sql, iter::empty(), out)?;
    }
    frontend::bind(
        "",
        "",
        iter::empty(),
        statement.params.iter(),
        |value, buffer| {
            Ok(match value {
                Some(text) => {
                    buffer.extend_from_slice(text.as_bytes());
                    IsNull::No
                }
                None => IsNull::Yes,
            })
        },
        formats.iter().copied(),
        out,
    )
    .map_err(|_| DriverError::Protocol("cannot encode Bind".to_owned()))?;
    frontend::execute("", 0, out)?;
    frontend::sync(out);
    Ok(())
}

impl Session for PgSession {
    fn connection(&mut self) -> Option<&mut dyn openrows_driver_api::Connection> {
        Some(self)
    }

    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run_guarded(sql, &[], sink))
    }

    fn execute_bound<'a>(
        &'a mut self,
        sql: &'a str,
        params: &'a [Option<String>],
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run_guarded(sql, params, sink))
    }

    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        self.cancel
            .clone()
            .map(|cancel| cancel as Arc<dyn CancelHandle>)
    }

    /// One simple-query round trip instead of Parse/Describe/Execute.
    fn ping(&mut self) -> BoxFuture<'_, Result<(), DriverError>> {
        Box::pin(async move { self.text_query("SELECT 1").await.map(drop) })
    }

    fn in_transaction(&self) -> bool {
        self.wire.in_transaction()
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        let mut wire = self.wire;
        frontend::terminate(wire.out());
        Box::pin(wire.shutdown())
    }
}
