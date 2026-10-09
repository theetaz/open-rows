// SPDX-License-Identifier: Apache-2.0
//! Atomic row edits and structure changes on an isolated connection: no user
//! query can interleave.
use crate::PgSession;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use openrows_driver_api::{DriverError, ResultSink, Session, SinkFlow};

/// A validated, parameterised row mutation, expected to affect exactly one row.
pub struct BoundEdit {
    /// Validated DML statement.
    pub sql: String,
    /// Text parameters; None is SQL NULL.
    pub params: Vec<Option<String>>,
}
struct Discard;
impl ResultSink for Discard {
    fn begin_result(&mut self, _schema: SchemaRef) {}
    fn push_batch(&mut self, _batch: RecordBatch) -> SinkFlow {
        SinkFlow::Stop
    }
}
impl PgSession {
    /// Commit all edits together, rolling back statement errors and row conflicts.
    /// Use a dedicated connection; errors retain the caller's staged edits.
    /// # Errors
    /// Server/transport errors, 40001 for a row conflict, or 08007 for an uncertain COMMIT.
    pub async fn commit_edits(&mut self, edits: &[BoundEdit]) -> Result<(), DriverError> {
        self.execute("BEGIN", &mut Discard)
            .await
            .map_err(DriverError::lost_before_commit)?;
        let result = self.apply_edits(edits).await;
        self.finish(result).await
    }

    /// Run structure-change statements (DDL) in one transaction: any failure
    /// rolls every statement back. Each runs through the extended protocol,
    /// which refuses more than one statement per text.
    /// # Errors
    /// Server/transport errors, or 08007 for an uncertain COMMIT.
    pub async fn commit_ddl(&mut self, statements: &[String]) -> Result<(), DriverError> {
        self.execute("BEGIN", &mut Discard)
            .await
            .map_err(DriverError::lost_before_commit)?;
        let mut result = Ok(());
        for sql in statements {
            result = self.execute_bound(sql, &[], &mut Discard).await.map(|_| ());
            if result.is_err() {
                break;
            }
        }
        self.finish(result).await
    }

    /// Roll back after a failure, else commit; a lost COMMIT reply is
    /// uncertain, a connection lost before COMMIT rolled everything back.
    async fn finish(&mut self, result: Result<(), DriverError>) -> Result<(), DriverError> {
        if let Err(error) = result {
            if error.is_connection_lost() {
                return Err(error.lost_before_commit());
            }
            self.execute("ROLLBACK", &mut Discard)
                .await
                .map_err(DriverError::lost_before_commit)?;
            return Err(error);
        }
        match self.execute("COMMIT", &mut Discard).await {
            Ok(_) => Ok(()),
            Err(error @ DriverError::Server { .. }) => {
                self.execute("ROLLBACK", &mut Discard).await?;
                Err(error)
            }
            Err(error) => Err(uncertain_commit(&error)),
        }
    }
    async fn apply_edits(&mut self, edits: &[BoundEdit]) -> Result<(), DriverError> {
        for edit in edits {
            let summary = self
                .execute_bound(&edit.sql, &edit.params, &mut Discard)
                .await?;
            let tag = summary.command_tag.as_str();
            if !matches!(tag, "UPDATE 1" | "DELETE 1" | "INSERT 0 1") {
                return Err(DriverError::Server { code: "40001".into(), message: "A row changed, was deleted, or matched more than once. All changes were rolled back; reload and review.".into() });
            }
        }
        Ok(())
    }
}

fn uncertain_commit(error: &DriverError) -> DriverError {
    DriverError::Server {
        code: "08007".into(),
        message: format!(
            "Commit outcome is unknown because its acknowledgement was lost: {error}. Reload the table before retrying; the changes may already be committed."
        ),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_lost_commit_acknowledgement_is_never_reported_as_rolled_back() {
        let error = uncertain_commit(&DriverError::Network(std::io::ErrorKind::BrokenPipe.into()));
        assert!(
            matches!(error, DriverError::Server { code, message } if code == "08007" && message.contains("may already be committed"))
        );
    }
}
