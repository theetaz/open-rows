// SPDX-License-Identifier: Apache-2.0
//! One facet fragment at a time; priority cancellation and Close wake its wait.
use crate::{flow::Flow, output::Output};
use openrows_driver_api::DriverError;
use openrows_host_protocol::{Control, FRAGMENT_LIMIT, FacetReply, Reply};
pub(crate) fn send(
    output: &Output,
    flow: &Flow,
    value: FacetReply,
    in_transaction: bool,
) -> Result<Reply, DriverError> {
    let bytes = value.encode().map_err(|_| {
        DriverError::Unsupported(
            "host facet exceeds 16 MiB, 65536 entries or nested collection limit".into(),
        )
    })?;
    drop(value); // Release native facet arrays before waiting for parent acknowledgements.
    let mut sequence = 0;
    for bytes in bytes.chunks(FRAGMENT_LIMIT) {
        sequence += 1;
        flow.offer_facet(sequence).map_err(|_| protocol())?;
        output
            .control(
                0,
                &Control::FacetChunk {
                    sequence,
                    bytes: bytes.to_vec(),
                },
            )
            .map_err(|_| protocol())?;
        if !flow.wait_facet().map_err(|_| protocol())? {
            return Err(DriverError::Server {
                code: "57014".into(),
                message: "catalog operation cancelled".into(),
            });
        }
    }
    Ok(Reply::FacetChunks {
        sequence,
        in_transaction,
    })
}
fn protocol() -> DriverError {
    DriverError::Host(openrows_driver_api::HostFailure::Protocol)
}
