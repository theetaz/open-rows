// SPDX-License-Identifier: Apache-2.0
//! Guarded edits execute once in the host; a lost acknowledgement is never replayed.
use crate::HostedConnection;
use openrows_driver_api::{BoxFuture, DriverError, Editor, RowEditInfo, RowSnapshot};
use openrows_host_protocol::{Control, EditorRequest, FacetReply, WireEdit};
use openrows_sql::select::BoundSql;
impl Editor for HostedConnection {
    fn row_edit_info<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<RowEditInfo, DriverError>> {
        Box::pin(async move {
            let request = EditorRequest::Info {
                schema: schema.into(),
                table: table.into(),
            };
            match self.facet(Control::Edit(request)).await? {
                FacetReply::EditInfo(value) => Ok(RowEditInfo {
                    key: value.key,
                    protected: value.protected,
                    row_id: value.row_id,
                }),
                _ => Err(self.protocol()),
            }
        })
    }
    fn edit_snapshot<'a>(
        &'a mut self,
        (schema, table): (&'a str, &'a str),
        keys: &'a [(String, String)],
    ) -> BoxFuture<'a, Result<RowSnapshot, DriverError>> {
        Box::pin(async move {
            let request = EditorRequest::Snapshot {
                schema: schema.into(),
                table: table.into(),
                keys: keys.to_vec(),
            };
            match self.facet(Control::Edit(request)).await? {
                FacetReply::Snapshot(value) => Ok(value),
                _ => Err(self.protocol()),
            }
        })
    }
    fn commit_edits<'a>(
        &'a mut self,
        edits: &'a [BoundSql],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move {
            let edits = edits
                .iter()
                .map(|edit| WireEdit {
                    sql: edit.sql.clone(),
                    params: edit.params.clone(),
                })
                .collect();
            self.committed(EditorRequest::Commit(edits)).await
        })
    }
    fn commit_ddl<'a>(
        &'a mut self,
        statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move {
            self.committed(EditorRequest::Ddl(statements.to_vec()))
                .await
        })
    }
}
impl HostedConnection {
    async fn committed(&mut self, request: EditorRequest) -> Result<(), DriverError> {
        let request = self.begin(&Control::Edit(request))?;
        let reply = self.finish_call(request).await.map_err(uncertain)?;
        match reply {
            openrows_host_protocol::Reply::Facet {
                value: FacetReply::Committed,
                in_transaction,
            } => {
                self.transaction = in_transaction;
                Ok(())
            }
            _ => Err(uncertain(self.protocol())),
        }
    }
}

fn uncertain(error: DriverError) -> DriverError {
    match error {
        DriverError::Host(_) => DriverError::Server {
            code: "08007".into(),
            message: "The driver process stopped before acknowledging the commit; its outcome is unknown. Refresh and reconcile before retrying.".into(),
        },
        other => other,
    }
}
