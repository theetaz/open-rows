// SPDX-License-Identifier: Apache-2.0
//! Monitor reads queued on a shared session's own connection (hosted
//! `DuckDB`, ADR-0010/0011): no second connection is opened, and a read that
//! cannot start while a user statement runs becomes a gap, never a stall.
use crate::FacetPort;
use openrows_driver_api::{BoxFuture, Connection, DriverError};
use openrows_monitor::{Descriptor, MonitorReader, ReadFuture, Reading, SampleState, Source};
use std::time::Duration;

/// A collector borrowing the owning session; closing it leaves that session open.
pub struct FacetMonitor {
    port: FacetPort,
    descriptors: Vec<Descriptor>,
}

impl FacetMonitor {
    /// Read through `port` with the engine's fixed `descriptors`; discovery
    /// never queues behind user statements.
    #[must_use]
    pub fn new(port: FacetPort, descriptors: Vec<Descriptor>) -> Self {
        Self { port, descriptors }
    }
}

impl MonitorReader for FacetMonitor {
    fn discover(&mut self) -> ReadFuture<'_, Vec<Descriptor>> {
        let descriptors = self.descriptors.clone();
        Box::pin(async move { descriptors })
    }

    fn read(&mut self, source: Source, deadline_ms: u32) -> ReadFuture<'_, Reading> {
        let port = self.port.clone();
        Box::pin(async move {
            // Leave the runtime's own deadline unreached: its timeout ends collection.
            let limit = Duration::from_millis(u64::from(deadline_ms) * 3 / 4);
            let queued = port.call_within(limit, move |connection| {
                monitor_read(connection, source, deadline_ms)
            });
            match queued.await {
                Ok(Some(reading)) => reading,
                Ok(None) => Reading::missing(SampleState::Gap),
                Err(error) => lost(&error),
            }
        })
    }

    /// Never cancels: the session's running statement belongs to the user.
    fn cancellation(&self) -> ReadFuture<'static, ()> {
        Box::pin(async {})
    }
}

fn monitor_read(
    connection: &mut dyn Connection,
    source: Source,
    deadline_ms: u32,
) -> BoxFuture<'_, Result<Reading, DriverError>> {
    match connection.monitor() {
        Some(reader) => Box::pin(async move { Ok(reader.read(source, deadline_ms).await) }),
        None => Box::pin(async {
            Err(DriverError::Unsupported(
                "this session has no monitor reader".into(),
            ))
        }),
    }
}

/// The owning session closed or its host failed: stop with an explicit reason.
fn lost(error: &DriverError) -> Reading {
    let reason = match error {
        DriverError::Host(failure) => format!("The owning session's {failure}"),
        _ => "The owning session closed or its driver host stopped; reconnect explicitly".into(),
    };
    let mut reading = Reading::missing(SampleState::Unavailable(reason));
    reading.terminal = true;
    reading
}
