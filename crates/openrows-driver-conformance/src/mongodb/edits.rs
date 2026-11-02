// SPDX-License-Identifier: Apache-2.0
use crate::{Failure, Target, ensure};
use openrows_driver_api::{DiscardSink, DriverError};
use openrows_sql::{
    Dialect,
    edit::{CellValue, Edit, EditKind, EditTable, edit_sql},
    select::TableRef,
};
use std::collections::BTreeMap;

pub(super) async fn run(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    connection
        .execute("db.orc_edits.deleteMany({})", &mut DiscardSink)
        .await?;
    connection
        .execute("db.orc_edits.insertOne({_id:1,n:2})", &mut DiscardSink)
        .await?;
    let snapshot = connection
        .editor()
        .ok_or_else(|| Failure::Contract("no editor".into()))?
        .edit_snapshot(
            (&target.schema, "orc_edits"),
            &[("_id".into(), "{\"$numberInt\":\"1\"}".into())],
        )
        .await?;
    let table = EditTable {
        table: TableRef {
            schema: target.schema.clone(),
            name: "orc_edits".into(),
        },
        columns: vec!["_id".into(), "__document".into()],
        key: vec!["_id".into()],
        protected: vec!["_id".into()],
    };
    let update = Edit {
        kind: EditKind::Update,
        original: snapshot.into_iter().collect(),
        values: BTreeMap::from([("__document".into(), CellValue::Text("{\"n\":3}".into()))]),
    };
    let sql = edit_sql(Dialect::MongoDb, &table, &update)
        .map_err(|e| Failure::Contract(e.to_string()))?;
    connection
        .editor()
        .ok_or_else(|| Failure::Contract("no editor".into()))?
        .commit_edits(std::slice::from_ref(&sql))
        .await?;
    let stale = connection
        .editor()
        .ok_or_else(|| Failure::Contract("no editor".into()))?
        .commit_edits(&[sql])
        .await;
    ensure(
        matches!(stale,Err(DriverError::Server{code,..}) if code == "40001"),
        || "stale document guard was accepted".into(),
    )?;
    connection
        .execute("db.orc_edits.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.close().await?;
    Ok(())
}
