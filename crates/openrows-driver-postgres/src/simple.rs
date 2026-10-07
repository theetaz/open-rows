// SPDX-License-Identifier: Apache-2.0
//! Simple-query helper for small internal queries and startup commands.
use crate::decode::next_cell;
use crate::wire::{Wire, server_error, unexpected};
use openrows_driver_api::DriverError;
use postgres_protocol::message::backend::Message;
use postgres_protocol::message::frontend;

/// One text-format row; `None` is SQL NULL.
pub(crate) type TextRow = Vec<Option<String>>;

/// Run `sql` (one or more `;`-separated statements) and collect every text row.
/// Always drains to `ReadyForQuery`, so a server error leaves the connection usable.
pub(crate) async fn query(wire: &mut Wire, sql: &str) -> Result<Vec<TextRow>, DriverError> {
    frontend::query(sql, wire.out())?;
    wire.flush().await?;
    let (mut rows, mut failure) = (Vec::new(), None);
    loop {
        match wire.next().await? {
            Message::DataRow(body) => rows.push(text_row(body.buffer())?),
            Message::ErrorResponse(body) => {
                failure.get_or_insert(server_error(&body));
            }
            Message::ReadyForQuery(_) => return failure.map_or(Ok(rows), Err),
            Message::RowDescription(_)
            | Message::CommandComplete(_)
            | Message::EmptyQueryResponse
            | Message::NoticeResponse(_)
            | Message::ParameterStatus(_) => {}
            _ => return Err(unexpected("simple query")),
        }
    }
}

fn text_row(mut row: &[u8]) -> Result<TextRow, DriverError> {
    let mut cells = Vec::new();
    while !row.is_empty() {
        let cell = next_cell(&mut row)
            .map_err(|_| DriverError::Protocol("malformed DataRow".to_owned()))?;
        cells.push(cell.map(|bytes| String::from_utf8_lossy(bytes).into_owned()));
    }
    Ok(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_rows_keep_nulls_and_reject_truncation() -> Result<(), DriverError> {
        let row = [0, 0, 0, 2, b'h', b'i', 255, 255, 255, 255];
        assert_eq!(text_row(&row)?, [Some("hi".to_owned()), None]);
        assert!(matches!(
            text_row(&[0, 0, 0, 5, b'x']),
            Err(DriverError::Protocol(_))
        ));
        Ok(())
    }
}
