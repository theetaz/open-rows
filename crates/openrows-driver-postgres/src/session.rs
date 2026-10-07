// SPDX-License-Identifier: Apache-2.0
//! Extended-protocol execution with per-column result formats.
use crate::cancel::PgCancel;
use crate::connect::establish;
use crate::decode::ResultDecoder;
use crate::info::ServerInfo;
use crate::options::ConnectOptions;
use crate::simple;
use crate::startup::PgConfig;
use crate::stream;
use crate::types::{self, ColumnDesc};
use crate::wire::{Wire, server_error, unexpected};
use fallible_iterator::FallibleIterator;
use openrows_driver_api::{
    BatchPolicy, BoxFuture, CancelHandle, DriverError, ExecSummary, ResultSink, Session,
};
use postgres_protocol::IsNull;
use postgres_protocol::message::backend::{Message, RowDescriptionBody};
use postgres_protocol::message::frontend;
use std::{collections::HashMap, fmt::Write, iter, sync::Arc};

/// One `PostgreSQL` connection, owned by a single session actor (ADR-0007).
pub struct PgSession {
    wire: Wire,
    type_names: HashMap<u32, String>,
    policy: BatchPolicy,
    cancel: Option<Arc<PgCancel>>,
    info: ServerInfo,
}

impl PgSession {
    /// Connect with default [`ConnectOptions`]: `sslmode=prefer` (plaintext
    /// until D-11), docs/14 §4 timeouts, no startup commands.
    ///
    /// # Errors
    /// As [`PgSession::connect_with`].
    pub async fn connect(config: &PgConfig) -> Result<Self, DriverError> {
        Self::connect_with(config, &ConnectOptions::default()).await
    }

    /// Connect, authenticate, read [`ServerInfo`] and run the startup commands.
    ///
    /// # Errors
    /// `Unsupported` for TLS-only `sslmode`s; `Network` for transport failures
    /// and stage timeouts (`io::ErrorKind::TimedOut`, message names the stage);
    /// `Auth`; `Server` when the server refuses the session or a startup
    /// command fails (message prefixed `startup command <n>:`); `Protocol`.
    pub async fn connect_with(
        config: &PgConfig,
        options: &ConnectOptions,
    ) -> Result<Self, DriverError> {
        let established = establish(config, options).await?;
        Ok(Self {
            wire: established.wire,
            type_names: HashMap::new(),
            policy: BatchPolicy::default(),
            cancel: established.cancel.map(Arc::new),
            info: established.info,
        })
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
        simple::query(&mut self.wire, sql).await
    }

    async fn run(
        &mut self,
        sql: &str,
        sink: &mut dyn ResultSink,
    ) -> Result<ExecSummary, DriverError> {
        let columns = self.describe(sql).await?;
        let reparse = self
            .resolve_type_names(columns.as_deref().unwrap_or_default())
            .await?;
        let decoder = columns.map(|c| ResultDecoder::new(&c, &self.type_names, self.policy));
        let formats = decoder
            .as_ref()
            .map_or(&[][..], ResultDecoder::result_formats);
        send_execute(self.wire.out(), sql, reparse, formats)?;
        self.wire.flush().await?;
        stream::collect(&mut self.wire, decoder, sink).await
    }

    /// Parse + Describe the unnamed statement; `None` when it returns no rows.
    async fn describe(&mut self, sql: &str) -> Result<Option<Vec<ColumnDesc>>, DriverError> {
        let out = self.wire.out();
        frontend::parse("", sql, iter::empty(), out)?;
        frontend::describe(b'S', "", out)?;
        frontend::sync(out);
        self.wire.flush().await?;
        let (mut columns, mut failure) = (None, None);
        loop {
            match self.wire.next().await? {
                Message::ParameterDescription(body) if body.parameters().count()? > 0 => {
                    failure.get_or_insert(DriverError::Unsupported("bind parameters".to_owned()));
                }
                Message::RowDescription(body) => columns = Some(column_descs(&body)?),
                Message::ErrorResponse(body) => failure = Some(server_error(&body)),
                Message::ReadyForQuery(_) => return failure.map_or(Ok(columns), Err),
                Message::ParseComplete
                | Message::ParameterDescription(_)
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

fn send_execute(
    out: &mut bytes::BytesMut,
    sql: &str,
    reparse: bool,
    formats: &[i16],
) -> Result<(), DriverError> {
    if reparse {
        frontend::parse("", sql, iter::empty(), out)?;
    }
    let no_values = iter::empty::<()>();
    frontend::bind(
        "",
        "",
        iter::empty(),
        no_values,
        |(), _| Ok(IsNull::Yes),
        formats.iter().copied(),
        out,
    )
    .map_err(|_| DriverError::Protocol("cannot encode Bind".to_owned()))?;
    frontend::execute("", 0, out)?;
    frontend::sync(out);
    Ok(())
}

impl Session for PgSession {
    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        sink: &'a mut dyn ResultSink,
    ) -> BoxFuture<'a, Result<ExecSummary, DriverError>> {
        Box::pin(self.run(sql, sink))
    }

    fn cancel_handle(&self) -> Option<Arc<dyn CancelHandle>> {
        self.cancel
            .clone()
            .map(|cancel| cancel as Arc<dyn CancelHandle>)
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        let mut wire = self.wire;
        frontend::terminate(wire.out());
        Box::pin(wire.shutdown())
    }
}
