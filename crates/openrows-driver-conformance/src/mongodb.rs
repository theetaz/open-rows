// SPDX-License-Identifier: Apache-2.0
//! Native document conformance, preserving the common capability contracts.
mod edits;
use crate::{Failure, Group, Target, ensure, sink::Collect};
use arrow_schema::DataType;
use openrows_driver_api::{ConnectStage, ConnectTrace, DiscardSink, DriverError, Feature};

pub(crate) async fn run(group: Group, target: &Target) -> Result<(), Failure> {
    match group {
        Group::Connect => connect(target).await,
        Group::Edits => edits::run(target).await,
        Group::Streaming => streaming(target).await,
        Group::Types => types(target).await,
        Group::Errors => errors(target).await,
        Group::Metadata => metadata(target).await,
        Group::Cancel => cancel(target).await,
        _ => Err(Failure::Contract(
            "unexpected MongoDB capability group".into(),
        )),
    }
}
async fn connect(target: &Target) -> Result<(), Failure> {
    let mut trace = ConnectTrace::default();
    let mut connection = target.driver.connect(&target.request, &mut trace).await?;
    ensure(
        trace
            .completed
            .iter()
            .any(|(stage, _)| *stage == ConnectStage::Authenticate),
        || "missing auth diagnostics".into(),
    )?;
    connection.ping().await?;
    ensure(
        connection.server().product == "MongoDB" && !connection.server().version.is_empty(),
        || "missing server identity".into(),
    )?;
    for (feature, offered) in [
        (Feature::Cancel, connection.cancel_handle().is_some()),
        (Feature::RowEdits, connection.editor().is_some()),
        (Feature::Catalog, connection.catalog().is_some()),
    ] {
        ensure(
            target.driver.capabilities().supports(feature) == offered,
            || format!("capability mismatch: {feature:?}"),
        )?;
    }
    connection.close().await?;
    Ok(())
}
async fn types(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    connection
        .execute("db.orc_types.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.execute("db.orc_types.insertOne({integer:{$numberLong:'9007199254740993'},nested:{value:1},items:[1,2],empty:'',nullValue:null})", &mut DiscardSink).await?;
    let mut sink = Collect::default();
    connection
        .execute("db.orc_types.find({})", &mut sink)
        .await?;
    let schema = sink
        .schemas
        .first()
        .ok_or_else(|| Failure::Contract("no schema".into()))?;
    for (name, expected) in [("integer", DataType::Int64), ("__document", DataType::Utf8)] {
        ensure(
            schema
                .field_with_name(name)
                .is_ok_and(|field| field.data_type() == &expected),
            || format!("wrong document type: {name}"),
        )?;
    }
    ensure(
        schema
            .field_with_name("nested")
            .is_ok_and(|f| matches!(f.data_type(), DataType::Struct(_))),
        || "missing Arrow struct".into(),
    )?;
    ensure(
        schema
            .field_with_name("items")
            .is_ok_and(|f| matches!(f.data_type(), DataType::List(_))),
        || "missing Arrow list".into(),
    )?;
    connection
        .execute("db.orc_types.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.close().await?;
    Ok(())
}
async fn streaming(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    connection
        .execute("db.orc_stream.deleteMany({})", &mut DiscardSink)
        .await?;
    let values = (0..10_000)
        .map(|n| format!("{{n:{n}}}"))
        .collect::<Vec<_>>()
        .join(",");
    connection
        .execute(
            &format!("db.orc_stream.insertMany([{values}])"),
            &mut DiscardSink,
        )
        .await?;
    let mut sink = Collect::default();
    let summary = connection
        .execute("db.orc_stream.find({}).sort({n:1})", &mut sink)
        .await?;
    ensure(
        summary.rows == 10_000 && sink.rows() == 10_000 && sink.schemas.len() == 1,
        || "incomplete streamed documents".into(),
    )?;
    ensure(
        sink.batches.first().is_some_and(|b| b.num_rows() <= 200),
        || "first batch too large".into(),
    )?;
    let mut early = Collect {
        stop_after: Some(1),
        ..Collect::default()
    };
    connection
        .execute("db.orc_stream.find({})", &mut early)
        .await?;
    ensure(early.batches.len() == 1, || "ignored sink stop".into())?;
    connection.ping().await?;
    connection
        .execute("db.orc_stream.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.close().await?;
    Ok(())
}
async fn errors(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    ensure(
        connection
            .execute("db.x.find({bad", &mut DiscardSink)
            .await
            .is_err(),
        || "invalid MQL accepted".into(),
    )?;
    connection
        .execute("db.orc_errors.deleteMany({})", &mut DiscardSink)
        .await?;
    connection
        .execute("db.orc_errors.insertOne({_id:1})", &mut DiscardSink)
        .await?;
    ensure(
        matches!(
            connection
                .execute("db.orc_errors.insertOne({_id:1})", &mut DiscardSink)
                .await,
            Err(DriverError::Server { .. })
        ),
        || "duplicate key did not fail".into(),
    )?;
    connection.ping().await?;
    connection
        .execute("db.orc_errors.deleteMany({})", &mut DiscardSink)
        .await?;
    let mut zero = Collect::default();
    connection
        .execute("db.orc_errors.countDocuments({})", &mut zero)
        .await?;
    ensure(zero.single()?.as_deref() == Some("0"), || {
        "empty collection count must be zero".into()
    })?;
    connection.close().await?;
    Ok(())
}
async fn metadata(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    connection
        .execute("db.orc_metadata.deleteMany({})", &mut DiscardSink)
        .await?;
    connection
        .execute("db.orc_metadata.insertOne({_id:1})", &mut DiscardSink)
        .await?;
    let catalog = connection
        .catalog()
        .ok_or_else(|| Failure::Contract("missing catalog".into()))?;
    ensure(
        catalog
            .list_databases()
            .await?
            .iter()
            .any(|db| db.name == target.request.database),
        || "missing database".into(),
    )?;
    ensure(
        catalog
            .search_index()
            .await?
            .iter()
            .any(|object| object.name == "orc_metadata"),
        || "missing collection".into(),
    )?;
    ensure(
        catalog
            .table_columns(&target.schema, "orc_metadata")
            .await?
            .iter()
            .any(|column| column.name == "_id" && column.primary_key),
        || "missing document key".into(),
    )?;
    connection
        .execute("db.orc_metadata.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.close().await?;
    Ok(())
}
async fn cancel(target: &Target) -> Result<(), Failure> {
    let mut connection = target.connect().await?;
    connection
        .execute("db.orc_cancel.deleteMany({})", &mut DiscardSink)
        .await?;
    connection
        .execute("db.orc_cancel.insertOne({_id:1})", &mut DiscardSink)
        .await?;
    let handle = connection
        .cancel_handle()
        .ok_or_else(|| Failure::Contract("no cancel handle".into()))?;
    let mut sink = Collect::default();
    let source = "db.orc_cancel.aggregate([{$project:{a:{$range:[0,1000000]}}},{$unwind:'$a'},{$group:{_id:null,n:{$sum:'$a'}}}])";
    let result = {
        let execution = connection.execute(source, &mut sink);
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => result,
            () = tokio::task::yield_now() => { handle.cancel().await?; execution.await }
        }
    };
    ensure(
        matches!(result,Err(DriverError::Server{code,..}) if code == "57014"),
        || "cancel did not return the cancel state".into(),
    )?;
    connection.ping().await?;
    connection
        .execute("db.orc_cancel.deleteMany({})", &mut DiscardSink)
        .await?;
    connection.close().await?;
    Ok(())
}
