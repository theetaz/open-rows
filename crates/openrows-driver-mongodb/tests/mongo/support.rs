// SPDX-License-Identifier: Apache-2.0
use openrows_driver_api::{
    ConnectRequest, ConnectTimeouts, DiscardSink, Endpoint, SslMode, TlsFiles,
};
use openrows_sql::edit::{CellValue, Edit, EditKind, EditTable, edit_sql};
use openrows_sql::{Dialect, select::TableRef};
use std::collections::BTreeMap;
pub(super) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(super) fn request() -> Result<ConnectRequest> {
    let port = std::env::var("OPENROWS_TEST_MONGO_PORT")?.parse()?;
    Ok(ConnectRequest {
        endpoint: Endpoint::Server {
            host: "127.0.0.1".into(),
            port,
        },
        database: "openrows_test".into(),
        user: String::new(),
        password: None,
        application_name: "openrows-mongodb-test".into(),
        ssl_mode: SslMode::Disable,
        tls: TlsFiles::default(),
        via: None,
        timeouts: ConnectTimeouts::default(),
        startup_commands: Vec::new(),
        driver_options: BTreeMap::from([("mongodb_direct".into(), "true".into())]),
    })
}
pub(super) async fn seed_catalog(connection: &mut dyn openrows_driver_api::Connection) -> Result {
    connection
        .execute("db.documents.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.execute("db.documents.insertMany([{_id:1,nested:{n:1},items:[1,2]},{_id:2,nested:{n:2},items:[]}])", &mut DiscardSink).await?;
    connection
        .execute(
            "db.documents.createIndex({nested:1},{name:'nested_test'})",
            &mut DiscardSink,
        )
        .await?;
    let structure = connection
        .catalog()
        .ok_or("missing catalog")?
        .table_structure("openrows_test", "documents")
        .await?;
    assert!(
        structure
            .indexes
            .iter()
            .any(|index| index.name == "nested_test")
    );
    connection
        .execute("db.documents.dropIndex('nested_test')", &mut DiscardSink)
        .await?;
    let aggregate = connection
        .execute(
            "db.documents.aggregate([{$match:{_id:1}},{$limit:1}])",
            &mut DiscardSink,
        )
        .await?;
    assert_eq!(aggregate.rows, 1);
    Ok(())
}
pub(super) async fn atomic_guards(connection: &mut dyn openrows_driver_api::Connection) -> Result {
    let snapshot = connection
        .editor()
        .ok_or("missing editor")?
        .edit_snapshot(
            ("openrows_test", "documents"),
            &[("_id".into(), "{\"$numberInt\":\"1\"}".into())],
        )
        .await?;
    let original: BTreeMap<_, _> = snapshot.into_iter().collect();
    let table = table();
    let update = Edit {
        kind: EditKind::Update,
        original,
        values: BTreeMap::from([(
            "__document".into(),
            CellValue::Text("{\"nested\":{\"n\":3},\"items\":[1,2]}".into()),
        )]),
    };
    connection
        .editor()
        .ok_or("missing editor")?
        .commit_edits(&[edit_sql(Dialect::MongoDb, &table, &update)?])
        .await?;
    let insert = Edit {
        kind: EditKind::Insert,
        original: BTreeMap::new(),
        values: BTreeMap::from([("__document".into(), CellValue::Text("{\"_id\":3}".into()))]),
    };
    let stale = edit_sql(Dialect::MongoDb, &table, &update)?;
    let result = connection
        .editor()
        .ok_or("missing editor")?
        .commit_edits(&[edit_sql(Dialect::MongoDb, &table, &insert)?, stale])
        .await;
    assert!(
        matches!(result, Err(openrows_driver_api::DriverError::Server {code,..}) if code == "40001")
    );
    assert_eq!(
        connection
            .execute("db.documents.find({_id:3})", &mut DiscardSink)
            .await?
            .rows,
        0
    );
    Ok(())
}
fn table() -> EditTable {
    EditTable {
        table: TableRef {
            schema: "openrows_test".into(),
            name: "documents".into(),
        },
        columns: vec!["_id".into(), "__document".into()],
        key: vec!["_id".into()],
        protected: vec!["_id".into()],
    }
}
pub(super) async fn insert_delete(connection: &mut dyn openrows_driver_api::Connection) -> Result {
    let table = table();
    let insert = Edit {
        kind: EditKind::Insert,
        original: BTreeMap::new(),
        values: BTreeMap::from([(
            "__document".into(),
            CellValue::Text("{\"_id\":3,\"n\":4}".into()),
        )]),
    };
    connection
        .editor()
        .ok_or("missing editor")?
        .commit_edits(&[edit_sql(Dialect::MongoDb, &table, &insert)?])
        .await?;
    let original = connection
        .editor()
        .ok_or("missing editor")?
        .edit_snapshot(
            ("openrows_test", "documents"),
            &[("_id".into(), "{\"$numberInt\":\"3\"}".into())],
        )
        .await?
        .into_iter()
        .collect();
    let delete = Edit {
        kind: EditKind::Delete,
        original,
        values: BTreeMap::new(),
    };
    connection
        .editor()
        .ok_or("missing editor")?
        .commit_edits(&[edit_sql(Dialect::MongoDb, &table, &delete)?])
        .await?;
    assert_eq!(
        connection
            .execute("db.documents.find({_id:3})", &mut DiscardSink)
            .await?
            .rows,
        0
    );
    let invalid = openrows_sql::select::BoundSql {
        sql: "db.documents.insertOne({v:{$oid:'invalid'}})".into(),
        params: vec![],
    };
    assert!(openrows_driver_mongodb::validate_edits(&[invalid]).is_err());
    Ok(())
}

pub(super) async fn mql_mutations(connection: &mut dyn openrows_driver_api::Connection) -> Result {
    connection
        .execute("db.mutations.deleteMany({})", &mut DiscardSink)
        .await?;
    connection
        .execute(
            "db.mutations.insertMany([{_id:1,n:1},{_id:2,n:1},{_id:3,n:2}])",
            &mut DiscardSink,
        )
        .await?;
    invalid_mutations(connection).await?;
    for (mutation, read, expected) in [
        (
            "db.mutations.updateOne({_id:1},{$inc:{n:1}})",
            "db.mutations.find({n:2})",
            2,
        ),
        (
            "db.mutations.updateMany({n:1},{$set:{n:3}})",
            "db.mutations.find({n:3})",
            1,
        ),
        ("db.mutations.deleteOne({n:2})", "db.mutations.find({})", 2),
        (
            "db.mutations.deleteMany({n:{$gte:2}})",
            "db.mutations.find({})",
            0,
        ),
    ] {
        connection.execute(mutation, &mut DiscardSink).await?;
        assert_eq!(
            connection.execute(read, &mut DiscardSink).await?.rows,
            expected
        );
    }
    Ok(())
}

async fn invalid_mutations(connection: &mut dyn openrows_driver_api::Connection) -> Result {
    for source in [
        "db.mutations.updateOne({_id:1},{})",
        "db.mutations.replaceOne({_id:1},{$set:{n:5}})",
    ] {
        assert!(connection.execute(source, &mut DiscardSink).await.is_err());
    }
    assert_eq!(
        connection
            .execute("db.mutations.find({n:1})", &mut DiscardSink)
            .await?
            .rows,
        2
    );
    Ok(())
}

pub(super) async fn null_identity(connection: &mut dyn openrows_driver_api::Connection) -> Result {
    connection
        .execute(
            "db.null_identity.insertOne({_id:null,n:1})",
            &mut DiscardSink,
        )
        .await?;
    let original = connection
        .editor()
        .ok_or("missing editor")?
        .edit_snapshot(
            ("openrows_test", "null_identity"),
            &[("_id".into(), "null".into())],
        )
        .await?
        .into_iter()
        .collect();
    let mut table = table();
    table.table.name = "null_identity".into();
    let update = Edit {
        kind: EditKind::Update,
        original,
        values: BTreeMap::from([("__document".into(), CellValue::Text("{\"n\":2}".into()))]),
    };
    connection
        .editor()
        .ok_or("missing editor")?
        .commit_edits(&[edit_sql(Dialect::MongoDb, &table, &update)?])
        .await?;
    assert_eq!(
        connection
            .execute("db.null_identity.find({_id:null,n:2})", &mut DiscardSink)
            .await?
            .rows,
        1
    );
    connection
        .execute("db.null_identity.deleteMany({})", &mut DiscardSink)
        .await?;
    Ok(())
}
