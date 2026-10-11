// SPDX-License-Identifier: Apache-2.0
//! Startup diagnostics contain only fixed stage names and protocol error categories.
use openrows_host_protocol::{Hello, ProtocolError};

pub(crate) fn contract(observed: &Hello, expected: &Hello) -> Result<(), ProtocolError> {
    let fields = [
        ("hello_version", observed.version == expected.version),
        ("hello_schema", observed.schema == expected.schema),
        ("hello_build", observed.build == expected.build),
        ("hello_driver", observed.driver == expected.driver),
        (
            "hello_generation",
            observed.generation == expected.generation,
        ),
        ("hello_limits", observed.limits == expected.limits),
        ("hello_arrow", observed.arrow == expected.arrow),
        ("hello_required", observed.required == expected.required),
        (
            "hello_capabilities",
            observed.capabilities == expected.capabilities,
        ),
    ];
    let name = fields
        .into_iter()
        .find(|(_, equal)| !equal)
        .map_or("hello_contract", |(name, _)| name);
    stage(name, observed.validate(expected))
}

#[allow(
    clippy::explicit_write,
    reason = "designated startup logging excludes all native and wire payloads"
)]
pub(crate) fn stage<T>(
    name: &'static str,
    result: Result<T, ProtocolError>,
) -> Result<T, ProtocolError> {
    result.inspect_err(|error| {
        let _ = std::io::Write::write_fmt(
            &mut std::io::stderr(),
            format_args!("OPENROWS_HOST_STARTUP:{name}:{error:?}\n"),
        );
    })
}
