// SPDX-License-Identifier: Apache-2.0
//! Bulk import through `COPY … FROM STDIN (FORMAT csv)` (FR-IO-02): rows are
//! encoded as CSV and streamed as `CopyData` chunks, so the server parses
//! them without a statement per row. A failing row fails the whole `COPY`.
use crate::PgSession;
use crate::wire::{Wire, server_error, unexpected};
use openrows_driver_api::{BoxFuture, BulkLoader, BulkTarget, DriverError, TextRow};
use openrows_sql::Dialect;
use postgres_protocol::message::backend::Message;
use postgres_protocol::message::frontend::{self, CopyData};

/// Encoded bytes sent per `CopyData` message and socket write.
const CHUNK: usize = 256 * 1024;

/// `COPY "schema"."table" ("a", "b") FROM STDIN (FORMAT csv)`.
fn copy_sql(target: BulkTarget<'_>) -> Result<String, DriverError> {
    let invalid = |_| DriverError::Unsupported("names containing NUL characters".to_owned());
    let table = Dialect::Postgres
        .qualified(target.schema, target.table)
        .map_err(invalid)?;
    let columns = target
        .columns
        .iter()
        .map(|name| Dialect::Postgres.quote_ident(name))
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?
        .join(", ");
    Ok(format!("COPY {table} ({columns}) FROM STDIN (FORMAT csv)"))
}

/// One CSV line: NULL is an unquoted empty field, every value is quoted (so
/// an empty string stays empty and `\.` is never an end marker).
fn encode_row(row: &TextRow, out: &mut Vec<u8>) -> Result<(), DriverError> {
    for (index, cell) in row.iter().enumerate() {
        if index > 0 {
            out.push(b',');
        }
        let Some(text) = cell else { continue };
        if text.contains('\0') {
            return Err(DriverError::Unsupported(
                "values containing NUL characters".to_owned(),
            ));
        }
        out.push(b'"');
        for byte in text.bytes() {
            if byte == b'"' {
                out.push(b'"');
            }
            out.push(byte);
        }
        out.push(b'"');
    }
    out.push(b'\n');
    Ok(())
}

/// Wait for `CopyInResponse`; a refused `COPY` drains to `ReadyForQuery`.
async fn copy_started(wire: &mut Wire) -> Result<(), DriverError> {
    loop {
        match wire.next().await? {
            Message::CopyInResponse(_) => return Ok(()),
            Message::ErrorResponse(body) => {
                let error = server_error(&body);
                ready(wire).await?;
                return Err(error);
            }
            Message::NoticeResponse(_) | Message::ParameterStatus(_) => {}
            _ => return Err(unexpected("COPY start")),
        }
    }
}

/// Read to `ReadyForQuery`, returning the `COPY n` count or the first error.
async fn copy_finished(wire: &mut Wire) -> Result<u64, DriverError> {
    let (mut count, mut failure) = (0, None);
    loop {
        match wire.next().await? {
            Message::CommandComplete(body) => {
                let tag = body.tag().map_err(|_| unexpected("COPY completion"))?;
                count = tag
                    .rsplit(' ')
                    .next()
                    .and_then(|number| number.parse().ok())
                    .unwrap_or(0);
            }
            Message::ErrorResponse(body) => {
                failure.get_or_insert(server_error(&body));
            }
            Message::ReadyForQuery(_) => return failure.map_or(Ok(count), Err),
            Message::NoticeResponse(_) | Message::ParameterStatus(_) => {}
            _ => return Err(unexpected("COPY completion")),
        }
    }
}

async fn ready(wire: &mut Wire) -> Result<(), DriverError> {
    loop {
        if let Message::ReadyForQuery(_) = wire.next().await? {
            return Ok(());
        }
    }
}

fn send_chunk(wire: &mut Wire, chunk: &mut Vec<u8>) -> Result<(), DriverError> {
    let data = CopyData::new(chunk.as_slice())?;
    data.write(wire.out());
    chunk.clear();
    Ok(())
}

impl PgSession {
    /// Stream `rows` into the target with `COPY FROM STDIN`; returns the
    /// server's row count. A value that cannot be sent aborts the `COPY`
    /// with `CopyFail`, so nothing of the call is stored.
    async fn copy_in(
        &mut self,
        target: BulkTarget<'_>,
        rows: Vec<TextRow>,
    ) -> Result<u64, DriverError> {
        let sql = copy_sql(target)?;
        frontend::query(&sql, self.wire.out())?;
        self.wire.flush().await?;
        copy_started(&mut self.wire).await?;
        let mut chunk = Vec::with_capacity(CHUNK + 1024);
        let mut encoded = Ok(());
        for row in &rows {
            encoded = encode_row(row, &mut chunk);
            if encoded.is_err() {
                break;
            }
            if chunk.len() >= CHUNK {
                send_chunk(&mut self.wire, &mut chunk)?;
                self.wire.flush().await?;
            }
        }
        if let Err(error) = encoded {
            frontend::copy_fail("the import stopped", self.wire.out())?;
            self.wire.flush().await?;
            // The server answers CopyFail with an error; report the cause.
            let _ = copy_finished(&mut self.wire).await;
            return Err(error);
        }
        send_chunk(&mut self.wire, &mut chunk)?;
        frontend::copy_done(self.wire.out());
        self.wire.flush().await?;
        copy_finished(&mut self.wire).await
    }
}

impl BulkLoader for PgSession {
    fn load<'a>(
        &'a mut self,
        target: BulkTarget<'a>,
        rows: Vec<TextRow>,
    ) -> BoxFuture<'a, Result<u64, DriverError>> {
        Box::pin(async move {
            let statement = self.begin_statement().await?;
            let result = self.copy_in(target, rows).await;
            self.finish_statement(statement, result).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_encode_as_quoted_csv_with_bare_nulls() -> Result<(), DriverError> {
        let mut out = Vec::new();
        let row = vec![
            Some("a\"b".to_owned()),
            None,
            Some(String::new()),
            Some("x,\ny".to_owned()),
        ];
        encode_row(&row, &mut out)?;
        assert_eq!(out, b"\"a\"\"b\",,\"\",\"x,\ny\"\n");
        let nul = vec![Some("a\0".to_owned())];
        assert!(matches!(
            encode_row(&nul, &mut Vec::new()),
            Err(DriverError::Unsupported(_))
        ));
        Ok(())
    }

    #[test]
    fn copy_names_are_quoted() -> Result<(), DriverError> {
        let columns = ["id".to_owned(), "Na\"me".to_owned()];
        let target = BulkTarget {
            schema: "public",
            table: "t",
            columns: &columns,
        };
        assert_eq!(
            copy_sql(target)?,
            "COPY \"public\".\"t\" (\"id\", \"Na\"\"me\") FROM STDIN (FORMAT csv)"
        );
        Ok(())
    }
}
