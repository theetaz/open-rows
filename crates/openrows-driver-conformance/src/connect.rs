// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Connect`]: connect, ping, failure kinds and capability
//! coherence.

use openrows_driver_api::objects::{ObjectChange, ObjectName};
use openrows_driver_api::{
    Capabilities, ConnectStage, ConnectTrace, Connection, DriverError, Endpoint, Feature,
    FileSource, Location,
};
use openrows_net::SecretString;

use crate::sink::ping;
use crate::{Failure, Target, ensure};

pub(crate) async fn run(target: &Target) -> Result<(), Failure> {
    let capabilities = target.driver.capabilities();
    let mut trace = ConnectTrace::default();
    let mut connection = target.driver.connect(&target.request, &mut trace).await?;
    ensure(
        trace.failed.is_none() && !trace.completed.is_empty(),
        || format!("connect trace {trace:?}"),
    )?;
    ensure(
        trace
            .completed
            .iter()
            .any(|(stage, _)| *stage == ConnectStage::Connect),
        || format!("no Connect stage recorded: {trace:?}"),
    )?;
    let product = connection.server().product.clone();
    ensure(!product.is_empty(), || "empty product name".to_owned())?;
    ensure(
        product == target.driver.product() || connection.server().flavour.is_some(),
        || {
            format!(
                "server says {product}, driver says {}",
                target.driver.product()
            )
        },
    )?;
    ping(connection.as_mut()).await?;
    coherence(target, &capabilities, connection.as_mut()).await?;
    connection.close().await?;
    startup_commands(target, &capabilities).await?;
    refused(target, &capabilities).await
}

/// Facets, cancel handle, structure changes, the schema fingerprint and
/// location match the claim.
async fn coherence(
    target: &Target,
    capabilities: &Capabilities,
    connection: &mut dyn Connection,
) -> Result<(), Failure> {
    let fingerprint = match connection.catalog() {
        Some(catalog) => catalog.schema_fingerprint().await?.is_some(),
        None => false,
    };
    let claims = [
        (Feature::Catalog, connection.catalog().is_some()),
        (Feature::RowEdits, connection.editor().is_some()),
        (Feature::ProcessList, connection.admin().is_some()),
        (Feature::BulkLoad, connection.bulk_loader().is_some()),
        (Feature::Cancel, connection.cancel_handle().is_some()),
        (Feature::StructureChanges, target.driver.supports_alter()),
        (Feature::TableLifecycle, lifecycle_offered(target)),
        (Feature::SchemaFingerprint, fingerprint),
    ];
    for (feature, offered) in claims {
        let claimed = capabilities.supports(feature);
        ensure(claimed == offered, || {
            format!("capability {feature:?}: claimed {claimed}, offered {offered}")
        })?;
    }
    let file = matches!(target.request.endpoint, Endpoint::File { .. });
    ensure(file == (capabilities.location == Location::File), || {
        format!(
            "location {:?} but the request is {file}",
            capabilities.location
        )
    })?;
    ensure(
        capabilities.object_kinds.is_empty() != capabilities.supports(Feature::Catalog),
        || "a catalog must list object kinds, and only a catalog".to_owned(),
    )
}

/// `object_sql` generates a table drop (the default refuses every change).
fn lifecycle_offered(target: &Target) -> bool {
    let drop = ObjectChange::DropTable {
        table: ObjectName {
            schema: target.schema.clone(),
            name: "orc_probe".to_owned(),
        },
        cascade: false,
    };
    target.driver.object_sql(&drop, None).is_ok()
}

/// Startup commands run on connect, in order, and the first failing one
/// fails the connect with a `Server` error naming it.
async fn startup_commands(target: &Target, capabilities: &Capabilities) -> Result<(), Failure> {
    if !capabilities.supports(Feature::StartupCommands) {
        return Ok(());
    }
    let mut request = target.request.clone();
    request.startup_commands = vec!["SELECT 1".to_owned()];
    let mut trace = ConnectTrace::default();
    let mut connection = target.driver.connect(&request, &mut trace).await?;
    ensure(
        trace
            .completed
            .iter()
            .any(|(stage, _)| *stage == ConnectStage::StartupCommands),
        || format!("no StartupCommands stage recorded: {trace:?}"),
    )?;
    ping(connection.as_mut()).await?;
    connection.close().await?;
    request.startup_commands = vec!["SELECT 1".to_owned(), "SELEC 2".to_owned()];
    match target
        .driver
        .connect(&request, &mut ConnectTrace::default())
        .await
    {
        Err(DriverError::Server { message, .. }) => {
            ensure(message.starts_with("startup command 2:"), || {
                format!("a failing startup command reported {message:?}")
            })
        }
        Err(other) => Err(Failure::Contract(format!(
            "a failing startup command: unexpected error kind: {other}"
        ))),
        Ok(_) => Err(Failure::Contract(
            "a failing startup command connected".to_owned(),
        )),
    }
}

/// A wrong password is `Auth` at the authentication stage; a missing file
/// fails to open.
async fn refused(target: &Target, capabilities: &Capabilities) -> Result<(), Failure> {
    let mut request = target.request.clone();
    match (&mut request.endpoint, capabilities.location) {
        (
            Endpoint::File {
                source: FileSource::Disk { path, create, .. },
                ..
            },
            Location::File,
        ) => {
            *path = path.with_file_name("orc-missing-database-file");
            *create = false;
        }
        _ => request.password = Some(SecretString::from("orc-wrong-password")),
    }
    let mut trace = ConnectTrace::default();
    let result = target.driver.connect(&request, &mut trace).await;
    match (result, capabilities.location) {
        (Err(DriverError::Auth(_)), Location::Server) => {
            ensure(trace.failed == Some(ConnectStage::Authenticate), || {
                format!("auth failure recorded at {:?}", trace.failed)
            })
        }
        (Err(DriverError::Server { .. }), Location::File) => Ok(()),
        (Err(other), _) => Err(Failure::Contract(format!(
            "refused connect: unexpected error kind: {other}"
        ))),
        (Ok(_), _) => Err(Failure::Contract(
            "a wrong password or missing file connected".to_owned(),
        )),
    }
}
