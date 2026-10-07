// SPDX-License-Identifier: Apache-2.0
//! Extended-protocol execution with per-column result formats.
use crate::decode::{ResultDecoder, next_cell};
use crate::startup::{PgConfig, startup};
use crate::stream;
use crate::types::{self, ColumnDesc};
use crate::wire::{Wire, server_error, unexpected};
use fallible_iterator::FallibleIterator;
use openrows_driver_api::{BatchPolicy, BoxFuture, DriverError, ExecSummary, ResultSink, Session};
use postgres_protocol::IsNull;
use postgres_protocol::message::backend::{Message, RowDescriptionBody};
use postgres_protocol::message::frontend;
use std::{collections::HashMap, fmt::Write, iter};

/// One `PostgreSQL` connection, owned by a single session actor (ADR-0007).
pub struct PgSession {
    wire: Wire,
    type_names: HashMap<u32, String>,
    policy: BatchPolicy,
}

impl PgSession {
    /// Connect and authenticate over plain TCP.
    ///
    /// # Errors
    /// `Network`, `Auth`, `Server` or `Protocol` failures during startup.
    pub async fn connect(config: &PgConfig) -> Result<Self, DriverError> {
        let mut wire = Wire::connect(&config.host, config.port).await?;
        startup(&mut wire, config).await?;
        Ok(Self {
            wire,
            type_names: HashMap::new(),
            policy: BatchPolicy::default(),
        })
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
        if let Some(decoder) = &decoder {
            sink.begin_result(decoder.schema());
        }
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
        frontend::query(&sql, self.wire.out())?;
        self.wire.flush().await?;
        let mut failure = None;
        loop {
            match self.wire.next().await? {
                Message::DataRow(body) => self.record_type_name(body.buffer())?,
                Message::ErrorResponse(body) => failure = Some(server_error(&body)),
                Message::ReadyForQuery(_) => return failure.map_or(Ok(true), Err),
                Message::RowDescription(_)
                | Message::CommandComplete(_)
                | Message::NoticeResponse(_)
                | Message::ParameterStatus(_) => {}
                _ => return Err(unexpected("type lookup")),
            }
        }
    }

    fn record_type_name(&mut self, mut row: &[u8]) -> Result<(), DriverError> {
        let invalid = || DriverError::Protocol("invalid type lookup row".to_owned());
        let mut text = || -> Result<&str, DriverError> {
            let cell = next_cell(&mut row).ok().flatten().ok_or_else(invalid)?;
            std::str::from_utf8(cell).map_err(|_| invalid())
        };
        let oid = text()?.parse().map_err(|_| invalid())?;
        let name = text()?.to_owned();
        self.type_names.insert(oid, name);
        Ok(())
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

    fn close(self: Box<Self>) -> BoxFuture<'static, Result<(), DriverError>> {
        let mut wire = self.wire;
        frontend::terminate(wire.out());
        Box::pin(wire.shutdown())
    }
}
