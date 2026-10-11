// SPDX-License-Identifier: Apache-2.0
//! Explicit stage and driver-error conversion, with no payload in host diagnostics.
use crate::HostFailure;
use openrows_driver_api::{ConnectStage, ConnectTrace, DriverError, TlsStatus};
use openrows_host_protocol::{WireError, WireTrace};
pub(crate) fn decode(trace: WireTrace) -> Result<ConnectTrace, DriverError> {
    Ok(ConnectTrace {
        completed: trace
            .completed
            .into_iter()
            .map(|(id, ns)| Ok((stage(id)?, std::time::Duration::from_nanos(ns))))
            .collect::<Result<_, DriverError>>()?,
        failed: trace.failed.map(stage).transpose()?,
        tls: trace.local_tls.then_some(TlsStatus::LocalSocket),
        retried_with_tls: trace.retried_tls,
    })
}
fn stage(id: u8) -> Result<ConnectStage, DriverError> {
    match id {
        1 => Ok(ConnectStage::Resolve),
        2 => Ok(ConnectStage::Connect),
        3 => Ok(ConnectStage::Tls),
        4 => Ok(ConnectStage::Authenticate),
        5 => Ok(ConnectStage::ServerInfo),
        6 => Ok(ConnectStage::StartupCommands),
        _ => Err(DriverError::Host(HostFailure::Protocol)),
    }
}
pub(crate) fn error(error: WireError) -> DriverError {
    match error {
        WireError::Server { code, message } => DriverError::Server { code, message },
        WireError::Unsupported(message) => DriverError::Unsupported(message),
        WireError::Decode { column, reason } => DriverError::Decode {
            column: column as usize,
            reason,
        },
        WireError::Auth => DriverError::Auth("host authentication refused".into()),
        WireError::Tls => DriverError::Tls("host TLS failed".into()),
        WireError::Network => DriverError::Network(std::io::ErrorKind::BrokenPipe.into()),
        WireError::Host(reason) => DriverError::Host(match reason {
            1 => HostFailure::Spawn,
            2 => {
                tracing::warn!(
                    check = "wire_host_failure",
                    observed = reason,
                    "driver host declared an incompatible connection"
                );
                HostFailure::Incompatible
            }
            3 => HostFailure::Exited,
            4 => HostFailure::MemoryLimit,
            5 => HostFailure::Unresponsive,
            // ID 6 and invalid IDs both poison the host as Protocol.
            _ => HostFailure::Protocol,
        }),
        WireError::Protocol => DriverError::Host(HostFailure::Protocol),
    }
}

impl crate::HostedConnection {
    pub(crate) fn remote_error(&self, wire: WireError) -> DriverError {
        let error = error(wire);
        if let DriverError::Host(failure) = &error {
            self.host.poison(*failure);
        }
        error
    }
}
