// SPDX-License-Identifier: Apache-2.0
//! Execution readiness must distinguish Parse/Describe from the sleeping query.
use super::*;
use openrows_driver_api::BoxFuture;

#[derive(Default)]
struct Listing {
    reads: usize,
    signalled: bool,
}

impl Admin for Listing {
    fn own_process(&mut self) -> BoxFuture<'_, Result<ProcessId, DriverError>> {
        Box::pin(async { Ok(1) })
    }

    fn list_processes(&mut self) -> BoxFuture<'_, Result<Vec<ProcessInfo>, DriverError>> {
        self.reads += 1;
        let (state, wait) = match self.reads {
            1 => ("active", None), // Parse has the SQL, but Execute has not arrived.
            2 => ("idle", Some("Client: ClientRead")),
            _ => ("active", Some("Timeout: PgSleep")),
        };
        Box::pin(async move {
            Ok(vec![ProcessInfo {
                id: 2,
                state: Some(state.into()),
                wait: wait.map(str::to_owned),
                query: Some("SELECT pg_sleep(60)".into()),
                ..ProcessInfo::default()
            }])
        })
    }

    fn signal_process(
        &mut self,
        _id: ProcessId,
        _signal: ProcessSignal,
    ) -> BoxFuture<'_, Result<(), DriverError>> {
        let executing = self.reads >= 3;
        self.signalled = executing;
        Box::pin(async move {
            if executing {
                Ok(())
            } else {
                Err(DriverError::Protocol(
                    "cancel arrived before Execute".into(),
                ))
            }
        })
    }
}

#[tokio::test]
async fn cancellation_waits_for_execute_instead_of_parse_or_idle_query_text() -> Result<(), Failure>
{
    let mut listing = Listing::default();
    cancel_when_listed(&mut listing, Dialect::Postgres, 2).await?;
    assert!(listing.signalled);
    assert_eq!(listing.reads, 3);
    Ok(())
}
