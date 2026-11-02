// SPDX-License-Identifier: Apache-2.0
//! Staged mutations execute once; retain final commit ambiguity from the SDK.
use crate::{MongoSession, command, error};
use mongodb::{
    ClientSession,
    bson::{Bson, doc},
};
use openrows_driver_api::{BoxFuture, DriverError, Editor, RowEditInfo, RowSnapshot};
use openrows_sql::select::BoundSql;

impl Editor for MongoSession {
    fn row_edit_info<'a>(
        &'a mut self,
        _schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<RowEditInfo, DriverError>> {
        Box::pin(async move {
            if self.is_view(table).await? {
                return Ok(RowEditInfo::default());
            }
            Ok(RowEditInfo {
                key: vec!["_id".into()],
                protected: vec!["_id".into()],
                row_id: None,
            })
        })
    }
    fn edit_snapshot<'a>(
        &'a mut self,
        target: (&'a str, &'a str),
        keys: &'a [(String, String)],
    ) -> BoxFuture<'a, Result<RowSnapshot, DriverError>> {
        Box::pin(async move {
            let [(name, value)] = keys else {
                return Err(error::invalid());
            };
            if name != "_id" {
                return Err(error::invalid());
            }
            let value: serde_json::Value =
                serde_json::from_str(value).map_err(|_| error::invalid())?;
            let id = Bson::try_from(value).map_err(|_| error::invalid())?;
            let document = self
                .database()
                .collection::<mongodb::bson::Document>(target.1)
                .find_one(doc! {"_id": id})
                .await
                .map_err(error::mongo)?
                .ok_or_else(conflict)?;
            let id = document
                .get("_id")
                .ok_or_else(error::invalid)?
                .clone()
                .into_canonical_extjson()
                .to_string();
            let document = Bson::Document(document)
                .into_canonical_extjson()
                .to_string();
            Ok(vec![
                ("_id".into(), Some(id)),
                ("__document".into(), Some(document)),
            ])
        })
    }
    fn commit_edits<'a>(
        &'a mut self,
        edits: &'a [BoundSql],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move {
            let commands = edits
                .iter()
                .map(staged_command)
                .collect::<Result<Vec<_>, _>>()?;
            let mut session = self.client.start_session().await.map_err(error::mongo)?;
            session.start_transaction().await.map_err(error::mongo)?;
            let result = self.apply(&commands, &mut session).await;
            if let Err(error) = result {
                session.abort_transaction().await.map_err(error::mongo)?;
                return Err(error);
            }
            session.commit_transaction().await.map_err(commit_error)
        })
    }
    fn commit_ddl<'a>(
        &'a mut self,
        _statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async {
            Err(DriverError::Unsupported(
                "MongoDB DDL is not transactional; run index commands in the MQL editor".into(),
            ))
        })
    }
}
impl MongoSession {
    async fn apply(
        &self,
        commands: &[mongodb::bson::Document],
        session: &mut ClientSession,
    ) -> Result<(), DriverError> {
        for command in commands {
            let reply = self
                .database()
                .run_command(command.clone())
                .session(&mut *session)
                .await
                .map_err(error::mongo)?;
            command::check_reply(&reply)?;
            if reply.get_i32("n").unwrap_or_default() != 1 {
                return Err(conflict());
            }
        }
        Ok(())
    }
}
fn conflict() -> DriverError {
    DriverError::Server {
        code: "40001".into(),
        message: "The MongoDB document changed or no longer exists; refresh before committing"
            .into(),
    }
}

fn staged_command(edit: &BoundSql) -> Result<mongodb::bson::Document, DriverError> {
    if !edit.params.is_empty() {
        return Err(error::invalid());
    }
    let query = openrows_sql::mql::parse(&edit.sql)
        .map_err(|error| DriverError::Unsupported(error.to_string()))?;
    if !matches!(
        query.method.as_str(),
        "insertOne" | "replaceOne" | "deleteOne"
    ) {
        return Err(error::invalid());
    }
    if query.method != "insertOne" {
        crate::validation::guard(&query)?;
    }
    command::command(&query)
}

/// Validate every staged EJSON document before a review token is issued.
/// # Errors
/// Invalid Extended JSON, unsupported operations or incomplete edit guards.
pub fn validate_edits(edits: &[BoundSql]) -> Result<(), DriverError> {
    edits
        .iter()
        .try_for_each(|edit| staged_command(edit).map(drop))
}

fn commit_error(error: mongodb::error::Error) -> DriverError {
    if error.contains_label("UnknownTransactionCommitResult")
        || !matches!(error.kind.as_ref(), mongodb::error::ErrorKind::Command(_))
    {
        return DriverError::Server {
            code: "08007".into(),
            message: "MongoDB commit outcome is unknown; verify before retrying".into(),
        };
    }
    error::mongo(error)
}
