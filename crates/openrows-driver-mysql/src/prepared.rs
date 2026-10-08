// SPDX-License-Identifier: Apache-2.0
//! Server-side prepared statements for bound text parameters: values never
//! enter the SQL text. Each statement is prepared, executed once and closed.
use mysql_common::io::ParseBuf;
use mysql_common::packets::{ComStmtClose, ComStmtExecuteRequestBuilder, StmtPacket};
use mysql_common::proto::MySerialize;
use mysql_common::value::Value;
use openrows_driver_api::DriverError;

use crate::error::{err_packet, unexpected};
use crate::wire::{Outcome, Protocol, Rows, Wire};

const COM_STMT_PREPARE: u8 = 0x16;

/// A prepared statement's id and parameter count.
struct Prepared {
    id: u32,
    params: usize,
}

impl Wire {
    /// Prepare `sql`, execute it with `params` (`None` is NULL), close it.
    ///
    /// # Errors
    /// Server errors (the connection stays usable), a parameter count
    /// mismatch (SQLSTATE `07001`), transport and protocol failures.
    pub(crate) async fn execute_bound(
        &mut self,
        sql: &str,
        params: &[Option<String>],
        rows: &mut dyn Rows,
    ) -> Result<Outcome, DriverError> {
        let statement = self.prepare(sql).await?;
        let result = self.execute(&statement, params, rows).await;
        let mut close = Vec::new();
        ComStmtClose::new(statement.id).serialize(&mut close);
        self.packets.reset();
        // COM_STMT_CLOSE has no reply.
        self.packets.write(&close).await?;
        result
    }

    async fn prepare(&mut self, sql: &str) -> Result<Prepared, DriverError> {
        self.command(COM_STMT_PREPARE, sql.as_bytes()).await?;
        let packet = self.packets.read().await?;
        if packet.first() == Some(&0xff) {
            return Err(err_packet(&packet, self.capabilities)
                .unwrap_or_else(|| unexpected("after COM_STMT_PREPARE", &packet)));
        }
        let ok = ParseBuf(&packet)
            .parse::<StmtPacket>(())
            .map_err(|error| DriverError::Protocol(format!("prepare reply: {error}")))?;
        let params = ok.num_params();
        self.columns(u64::from(params)).await?;
        self.columns(u64::from(ok.num_columns())).await?;
        Ok(Prepared {
            id: ok.statement_id(),
            params: usize::from(params),
        })
    }

    async fn execute(
        &mut self,
        statement: &Prepared,
        params: &[Option<String>],
        rows: &mut dyn Rows,
    ) -> Result<Outcome, DriverError> {
        if params.len() != statement.params {
            return Err(DriverError::Server {
                code: "07001".to_owned(),
                message: format!(
                    "the statement has {} parameters but {} values were given",
                    statement.params,
                    params.len()
                ),
            });
        }
        let values: Vec<Value> = params
            .iter()
            .map(|value| {
                value
                    .as_ref()
                    .map_or(Value::NULL, |text| Value::Bytes(text.clone().into_bytes()))
            })
            .collect();
        let (request, long_data) = ComStmtExecuteRequestBuilder::new(statement.id).build(&values);
        if long_data {
            return Err(DriverError::Unsupported(
                "parameter values larger than 16 MiB in total".to_owned(),
            ));
        }
        let mut payload = Vec::new();
        request.serialize(&mut payload);
        self.packets.reset();
        self.packets.write(&payload).await?;
        self.results(Protocol::Binary, rows).await
    }
}
