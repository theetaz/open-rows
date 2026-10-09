// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Connect`]: connect, ping, failure kinds and capability
//! coherence.

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
    coherence(target, &capabilities, connection.as_mut())?;
    connection.close().await?;
    refused(target, &capabilities).await
}

/// Facets, cancel handle, structure changes and location match the claim.
fn coherence(
    target: &Target,
    capabilities: &Capabilities,
    connection: &mut dyn Connection,
) -> Result<(), Failure> {
    let claims = [
        (Feature::Catalog, connection.catalog().is_some()),
        (Feature::RowEdits, connection.editor().is_some()),
        (Feature::Cancel, connection.cancel_handle().is_some()),
        (Feature::StructureChanges, target.driver.supports_alter()),
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
