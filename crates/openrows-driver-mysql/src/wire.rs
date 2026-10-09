// SPDX-License-Identifier: Apache-2.0
//! Commands and their results on a logged-in connection: `COM_QUERY` (text
//! rows), `COM_STMT_PREPARE`/`EXECUTE`/`CLOSE` (binary rows) and `COM_QUIT`.
//! Every result set of a command (stored procedures return several) goes to
//! the same [`Rows`] consumer.
use mysql_common::constants::{CapabilityFlags, StatusFlags};
use mysql_common::io::ParseBuf;
use mysql_common::packets::{
    Column, CommonOkPacket, OkPacket, OkPacketDeserializer, OldEofPacket, ResultSetTerminator,
};
use openrows_driver_api::DriverError;

use crate::error::{err_packet, unexpected};
use crate::packet::Packets;
use crate::values::{binary_row, lenenc, text_row};

const COM_QUIT: u8 = 0x01;
const COM_QUERY: u8 = 0x03;

/// Receives every result set of one command.
pub(crate) trait Rows: Send {
    /// A result set with these columns starts.
    ///
    /// # Errors
    /// The consumer refuses the columns.
    fn begin(&mut self, columns: &[Column]) -> Result<(), DriverError>;

    /// One row in text form (`None` is NULL); `false` stops decoding (the
    /// rest of the result is read and dropped).
    ///
    /// # Errors
    /// A value the consumer cannot decode.
    fn row(&mut self, values: Vec<Option<Vec<u8>>>) -> Result<bool, DriverError>;

    /// The current result set ended.
    ///
    /// # Errors
    /// The consumer fails to finish the result.
    fn end(&mut self) -> Result<(), DriverError>;
}

/// Row encoding of a command's result sets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Protocol {
    Text,
    Binary,
}

/// What a command reported besides rows.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Outcome {
    /// Rows changed (or matched, with `CLIENT_FOUND_ROWS`) by the last statement.
    pub(crate) affected: u64,
    /// Warnings raised (see `SHOW WARNINGS`).
    pub(crate) warnings: u16,
}

/// A logged-in connection.
pub(crate) struct Wire {
    pub(crate) packets: Packets,
    pub(crate) capabilities: CapabilityFlags,
    /// `SERVER_STATUS_IN_TRANS` of the last OK or EOF packet.
    pub(crate) in_transaction: bool,
}

impl Wire {
    /// Send one command packet, starting a new sequence.
    pub(crate) async fn command(&mut self, code: u8, body: &[u8]) -> Result<(), DriverError> {
        self.packets.reset();
        let mut payload = Vec::with_capacity(body.len() + 1);
        payload.push(code);
        payload.extend_from_slice(body);
        self.packets.write(&payload).await
    }

    /// Run `sql` as `COM_QUERY`.
    pub(crate) async fn query(
        &mut self,
        sql: &str,
        rows: &mut dyn Rows,
    ) -> Result<Outcome, DriverError> {
        self.command(COM_QUERY, sql.as_bytes()).await?;
        self.results(Protocol::Text, rows).await
    }

    /// Say goodbye and close the transport.
    pub(crate) async fn quit(&mut self) -> Result<(), DriverError> {
        self.command(COM_QUIT, &[]).await?;
        self.packets.shutdown().await
    }

    fn deprecate_eof(&self) -> bool {
        self.capabilities
            .contains(CapabilityFlags::CLIENT_DEPRECATE_EOF)
    }

    /// Read result sets until the server says no more follow. An error
    /// result ends the command; the connection stays usable.
    pub(crate) async fn results(
        &mut self,
        protocol: Protocol,
        rows: &mut dyn Rows,
    ) -> Result<Outcome, DriverError> {
        let mut outcome = Outcome::default();
        loop {
            let packet = self.packets.read().await?;
            let status = match packet.first() {
                Some(0x00) => {
                    let ok = self.ok_packet::<CommonOkPacket>(&packet)?;
                    outcome.affected = ok.affected_rows();
                    outcome.warnings = ok.warnings();
                    ok.status_flags()
                }
                Some(0xff) => match err_packet(&packet, self.capabilities) {
                    Some(error) => return Err(error),
                    None => continue,
                },
                Some(0xfb) => return Err(self.refuse_local_infile().await),
                _ => {
                    let (status, warnings) = self.result_set(&packet, protocol, rows).await?;
                    outcome.warnings = warnings;
                    status
                }
            };
            self.in_transaction = status.contains(StatusFlags::SERVER_STATUS_IN_TRANS);
            if !status.contains(StatusFlags::SERVER_MORE_RESULTS_EXISTS) {
                return Ok(outcome);
            }
        }
    }

    /// `LOAD DATA LOCAL INFILE` is never allowed: send no file and report it.
    async fn refuse_local_infile(&mut self) -> DriverError {
        if let Err(error) = self.packets.write(&[]).await {
            return error;
        }
        // The server answers the empty file with OK or ERR; either way the
        // statement did not load local data.
        let _ = self.packets.read().await;
        DriverError::Unsupported("LOAD DATA LOCAL INFILE (reading local files)".to_owned())
    }

    async fn result_set(
        &mut self,
        header: &[u8],
        protocol: Protocol,
        rows: &mut dyn Rows,
    ) -> Result<(StatusFlags, u16), DriverError> {
        let (count, _) = lenenc(header)?;
        let columns = self.columns(count).await?;
        rows.begin(&columns)?;
        let mut reader = RowReader {
            protocol,
            decoding: true,
            failure: None,
        };
        loop {
            let packet = self.packets.read().await?;
            if self.is_terminator(&packet) {
                reader.finish(rows)?;
                let ok = self.terminator(&packet)?;
                return Ok((ok.status_flags(), ok.warnings()));
            }
            if packet.first() == Some(&0xff) {
                match err_packet(&packet, self.capabilities) {
                    Some(error) => return Err(error),
                    None => continue,
                }
            }
            reader.row(&packet, &columns, rows)?;
        }
    }

    /// `count` column definitions, then the old EOF packet if not deprecated.
    pub(crate) async fn columns(&mut self, count: u64) -> Result<Vec<Column>, DriverError> {
        let mut columns = Vec::new();
        for _ in 0..count {
            let packet = self.packets.read().await?;
            let column = ParseBuf(&packet)
                .parse::<Column>(())
                .map_err(|error| DriverError::Protocol(format!("column definition: {error}")))?;
            columns.push(column);
        }
        if count > 0 && !self.deprecate_eof() {
            let eof = self.packets.read().await?;
            if !self.is_terminator(&eof) {
                return Err(unexpected("after column definitions", &eof));
            }
        }
        Ok(columns)
    }

    fn is_terminator(&self, packet: &[u8]) -> bool {
        let limit = if self.deprecate_eof() { 0xff_ffff } else { 9 };
        packet.first() == Some(&0xfe) && packet.len() < limit
    }

    fn terminator(&self, packet: &[u8]) -> Result<OkPacket<'static>, DriverError> {
        if self.deprecate_eof() {
            self.ok_packet::<ResultSetTerminator>(packet)
        } else {
            self.ok_packet::<OldEofPacket>(packet)
        }
    }

    fn ok_packet<K>(&self, packet: &[u8]) -> Result<OkPacket<'static>, DriverError>
    where
        K: mysql_common::packets::OkPacketKind,
    {
        ParseBuf(packet)
            .parse::<OkPacketDeserializer<'_, K>>(self.capabilities)
            .map(|ok| OkPacket::from(ok).into_owned())
            .map_err(|error| DriverError::Protocol(format!("OK packet: {error}")))
    }
}

/// Decodes rows until the consumer stops or rejects a value; a rejected
/// value is kept and reported once the result has been read to its end, so
/// the connection stays in step.
struct RowReader {
    protocol: Protocol,
    decoding: bool,
    failure: Option<DriverError>,
}

impl RowReader {
    /// The kept failure, or the consumer's end of the result.
    fn finish(self, rows: &mut dyn Rows) -> Result<(), DriverError> {
        match self.failure {
            Some(error) => Err(error),
            None => rows.end(),
        }
    }

    fn row(
        &mut self,
        packet: &[u8],
        columns: &[Column],
        rows: &mut dyn Rows,
    ) -> Result<(), DriverError> {
        if !self.decoding {
            return Ok(());
        }
        let values = match self.protocol {
            Protocol::Text => text_row(packet, columns.len())?,
            Protocol::Binary => binary_row(packet, columns)?,
        };
        match rows.row(values) {
            Ok(more) => self.decoding = more,
            Err(error) => {
                self.failure = Some(error);
                self.decoding = false;
            }
        }
        Ok(())
    }
}
