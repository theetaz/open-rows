// SPDX-License-Identifier: Apache-2.0
//! `MongoDB` happy path and atomic document safety against a throwaway replica set.
use mongodb::bson::doc;
use openrows_driver_api::{ConnectTrace, DiscardSink, Driver, Endpoint, SslMode};
use openrows_driver_mongodb::MongoDriver;

mod support;
use support::{Result, atomic_guards, insert_delete, request, seed_catalog};
#[tokio::test]
#[ignore = "needs MongoDB replica set; run just test-mongo"]
async fn documents_catalog_indexes_aggregation_and_atomic_edits() -> Result {
    let request = request()?;
    let mut connection = MongoDriver
        .connect(&request, &mut ConnectTrace::default())
        .await?;
    seed_catalog(connection.as_mut()).await?;
    atomic_guards(connection.as_mut()).await?;
    insert_delete(connection.as_mut()).await?;
    connection
        .execute("db.documents.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.close().await?;
    Ok(())
}
#[tokio::test]
#[ignore = "needs MongoDB; run just test-mongo"]
async fn views_are_browsable_without_document_mutation_keys() -> Result {
    let request = request()?;
    let Endpoint::Server { port, .. } = request.endpoint else {
        return Err("missing endpoint".into());
    };
    let observer =
        mongodb::Client::with_uri_str(format!("mongodb://127.0.0.1:{port}/?directConnection=true"))
            .await?;
    observer
        .database(&request.database)
        .run_command(doc! {"create":"document_view","viewOn":"documents","pipeline":[]})
        .await?;
    let mut connection = MongoDriver
        .connect(&request, &mut ConnectTrace::default())
        .await?;
    let catalog = connection.catalog().ok_or("missing catalog")?;
    let views = catalog
        .list_objects(
            &request.database,
            &[openrows_driver_api::catalog::ObjectKind::View],
        )
        .await?;
    assert!(views.iter().any(|view| view.name == "document_view"));
    assert!(
        catalog
            .table_structure(&request.database, "document_view")
            .await?
            .indexes
            .is_empty()
    );
    assert!(
        connection
            .editor()
            .ok_or("missing editor")?
            .row_edit_info(&request.database, "document_view")
            .await?
            .key
            .is_empty()
    );
    connection
        .execute("db.document_view.find({}).toArray()", &mut DiscardSink)
        .await?;
    observer
        .database(&request.database)
        .run_command(doc! {"drop":"document_view"})
        .await?;
    connection.close().await?;
    observer.shutdown().await;
    Ok(())
}
#[tokio::test]
#[ignore = "needs MongoDB; run just test-mongo"]
async fn credentials_are_checked_and_errors_do_not_echo_values() -> Result {
    let request = request()?;
    let Endpoint::Server { port, .. } = request.endpoint else {
        return Err("missing endpoint".into());
    };
    let client =
        mongodb::Client::with_uri_str(format!("mongodb://127.0.0.1:{port}/?directConnection=true"))
            .await?;
    client.database("openrows_test").run_command(doc! {"createUser":"openrows_fixture","pwd":"synthetic-test-password","roles":["readWrite"]}).await?;
    let mut authenticated = request.clone();
    authenticated.user = "openrows_fixture".into();
    authenticated.password = Some(openrows_net::SecretString::from("synthetic-test-password"));
    let connection = MongoDriver
        .connect(&authenticated, &mut ConnectTrace::default())
        .await?;
    connection.close().await?;
    authenticated.password = Some(openrows_net::SecretString::from("synthetic-wrong-password"));
    let result = MongoDriver
        .connect(&authenticated, &mut ConnectTrace::default())
        .await;
    assert!(matches!(
        result,
        Err(openrows_driver_api::DriverError::Auth(_))
    ));
    client
        .database("openrows_test")
        .run_command(doc! {"dropUser":"openrows_fixture"})
        .await?;
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
#[ignore = "needs MongoDB TLS fixture; run just test-mongo"]
async fn tls_and_routed_sockets_use_the_shared_network_layer() -> Result {
    use openrows_net::{BoxFuture, BoxStream, Dialer, TcpDialer};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Observed {
        dialer: TcpDialer,
        count: Arc<AtomicUsize>,
    }
    impl Dialer for Observed {
        fn dial(&self) -> BoxFuture<'_, std::io::Result<BoxStream>> {
            self.count.fetch_add(1, Ordering::Relaxed);
            self.dialer.dial()
        }
        fn describe(&self) -> String {
            "MongoDB integration route".into()
        }
    }
    let mut request = request()?;
    let Endpoint::Server { port, .. } = request.endpoint else {
        return Err("missing endpoint".into());
    };
    request.ssl_mode = SslMode::VerifyFull;
    request.tls.ca_file = Some(std::env::var("OPENROWS_TEST_MONGO_CA")?.into());
    let count = Arc::new(AtomicUsize::new(0));
    request.via = Some(Arc::new(Observed {
        dialer: TcpDialer::new("127.0.0.1".into(), port),
        count: count.clone(),
    }));
    let mut trace = ConnectTrace::default();
    let mut connection = MongoDriver.connect(&request, &mut trace).await?;
    assert!(matches!(
        trace.tls,
        Some(openrows_driver_api::TlsStatus::Encrypted(_))
    ));
    connection.ping().await?;
    assert!(count.load(Ordering::Relaxed) > 1);
    connection.close().await?;
    request.tls.ca_file = None;
    let mut failed = ConnectTrace::default();
    assert!(matches!(
        MongoDriver.connect(&request, &mut failed).await,
        Err(openrows_driver_api::DriverError::Tls(_))
    ));
    assert_eq!(failed.failed, Some(openrows_driver_api::ConnectStage::Tls));
    Ok(())
}

#[tokio::test]
#[ignore = "needs MongoDB; run just test-mongo"]
async fn cancel_stops_an_active_server_operation_and_keeps_session_usable() -> Result {
    let request = request()?;
    let Endpoint::Server { port, .. } = request.endpoint else {
        return Err("missing endpoint".into());
    };
    let observer =
        mongodb::Client::with_uri_str(format!("mongodb://127.0.0.1:{port}/?directConnection=true"))
            .await?;
    observer
        .database("admin")
        .run_command(doc! {"ping":1})
        .await?;
    let mut connection = MongoDriver
        .connect(&request, &mut ConnectTrace::default())
        .await?;
    connection
        .execute("db.cancel_documents.deleteMany({})", &mut DiscardSink)
        .await?;
    let documents = (0..500)
        .map(|n| format!("{{n:{n}}}"))
        .collect::<Vec<_>>()
        .join(",");
    connection
        .execute(
            &format!("db.cancel_documents.insertMany([{documents}])"),
            &mut DiscardSink,
        )
        .await?;
    let handle = connection.cancel_handle().ok_or("missing cancel handle")?;
    let running = tokio::spawn(async move {
        let result = connection.execute("db.cancel_documents.aggregate([{$lookup:{from:'cancel_documents',pipeline:[],as:'one'}},{$unwind:'$one'},{$lookup:{from:'cancel_documents',pipeline:[],as:'two'}},{$unwind:'$two'},{$group:{_id:null,n:{$sum:1}}}])", &mut DiscardSink).await;
        (connection, result)
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), active(&observer, true)).await??;
    tokio::time::timeout(std::time::Duration::from_secs(10), handle.cancel()).await??;
    let (mut connection, result) =
        tokio::time::timeout(std::time::Duration::from_secs(10), running).await??;
    assert!(
        matches!(result,Err(openrows_driver_api::DriverError::Server{code,..}) if code == "57014")
    );
    tokio::time::timeout(std::time::Duration::from_secs(10), active(&observer, false)).await??;
    connection.ping().await?;
    connection
        .execute("db.cancel_documents.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.close().await?;
    observer.shutdown().await;
    Ok(())
}
async fn active(observer: &mongodb::Client, expected: bool) -> Result {
    loop {
        let reply = observer
            .database("admin")
            .run_command(doc! {"currentOp":1,"active":true,"command.aggregate":"cancel_documents"})
            .await?;
        if reply.get_array("inprog")?.is_empty() != expected {
            return Ok(());
        }
    }
}

#[tokio::test]
#[ignore = "needs MongoDB failpoints; run just test-mongo"]
async fn native_commit_retry_never_replays_the_mutation_batch() -> Result {
    let request = request()?;
    let Endpoint::Server { port, .. } = request.endpoint else {
        return Err("missing endpoint".into());
    };
    let observer =
        mongodb::Client::with_uri_str(format!("mongodb://127.0.0.1:{port}/?directConnection=true"))
            .await?;
    let mut connection = MongoDriver
        .connect(&request, &mut ConnectTrace::default())
        .await?;
    connection
        .execute("db.commit_retry.deleteMany({})", &mut DiscardSink)
        .await?;
    observer.database("admin").run_command(doc! {"configureFailPoint":"failCommand","mode":{"times":1},"data":{"failCommands":["commitTransaction"],"closeConnection":true}}).await?;
    let insert = openrows_sql::select::BoundSql {
        sql: "db.commit_retry.insertOne({n:1})".into(),
        params: vec![],
    };
    connection
        .editor()
        .ok_or("missing editor")?
        .commit_edits(&[insert])
        .await?;
    assert_eq!(
        connection
            .execute("db.commit_retry.find({})", &mut DiscardSink)
            .await?
            .rows,
        1
    );
    connection
        .execute("db.commit_retry.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.close().await?;
    observer.shutdown().await;
    Ok(())
}
