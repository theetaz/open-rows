// SPDX-License-Identifier: Apache-2.0
//! Priority delivery is measured with data withheld and an actually stalled reader.
use crate::support;
use openrows_host_client::{HostFailure, MemoryBudget};
use openrows_host_protocol::{Control, Reply, WireError};
use std::time::{Duration, Instant};
use support::*;

async fn ready_batch(
    session: &mut openrows_host_client::HostSession,
) -> Result<openrows_host_protocol::Grant, HostFailure> {
    loop {
        if let Control::BatchReady(grant) = receive(session).await?.1 {
            return Ok(grant);
        }
    }
}
async fn cancel_ack(
    session: &mut openrows_host_client::HostSession,
) -> Result<Duration, HostFailure> {
    let start = Instant::now();
    session.send(0, &Control::Cancel { target: 2 })?;
    loop {
        if matches!(
            receive(session).await?.1,
            Control::CancelAck { target: 2, .. }
        ) {
            return Ok(start.elapsed());
        }
    }
}
#[tokio::test]
async fn cancel_close_and_heartbeat_live_while_paused_or_data_full() -> Result<(), HostFailure> {
    let _permit = host_test().await?;
    for full in [false, true] {
        let mut session = host(MemoryBudget::default()).await?;
        session.send(
            2,
            &Control::Execute {
                sql: "select hex(zeroblob(1048576))".into(),
            },
        )?;
        let grant = ready_batch(&mut session).await?;
        if full {
            session.send(0, &Control::Grant(grant))?;
        } else {
            session.send(0, &Control::Pause(grant.key))?;
        }
        let mut heartbeat = session.heartbeats();
        tokio::time::timeout(Duration::from_secs(2), heartbeat.changed())
            .await
            .map_err(|_| HostFailure::Unresponsive)?
            .map_err(|_| HostFailure::Exited)?;
        let elapsed = cancel_ack(&mut session).await?;
        assert!(
            elapsed <= Duration::from_millis(500),
            "CancelAck {elapsed:?} exceeds generous CI margin (target 200 ms)"
        );
        writeln_metric(full, elapsed);
        let pid = session.pid();
        let directory = session.directory().to_path_buf();
        let close = session.close().await;
        assert!(matches!(
            close,
            Ok(()) | Err(HostFailure::Unresponsive | HostFailure::Exited)
        ));
        assert_reaped(pid, &directory);
    }
    Ok(())
}
#[allow(
    clippy::explicit_write,
    reason = "designated acceptance timing output avoids production debug logging"
)]
fn writeln_metric(full: bool, elapsed: Duration) {
    use std::io::Write;
    writeln!(
        std::io::stdout(),
        "CancelAck reader_stalled={full}: {:.3} ms (target <= 200 ms)",
        elapsed.as_secs_f64() * 1000.0
    )
    .unwrap();
}
#[tokio::test]
async fn pause_resume_consumption_and_late_cancel_cannot_affect_next_query()
-> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let mut session = host(MemoryBudget::default()).await?;
    session.send(
        2,
        &Control::Execute {
            sql: "select 1".into(),
        },
    )?;
    let grant = ready_batch(&mut session).await?;
    session.send(0, &Control::Pause(grant.key))?;
    assert!(matches!(
        receive(&mut session).await?.1,
        Control::Paused { .. }
    ));
    session.send(0, &Control::Resume(grant))?;
    let mut decoder = openrows_host_protocol::ArrowDecoder::new();
    let bytes = data(&mut session, grant.encoded as usize).await?;
    assert_eq!(
        decoder
            .admit(&bytes, grant)
            .map_err(HostFailure::from)?
            .unwrap()
            .num_rows(),
        1
    );
    session.send(0, &Control::Consumed(grant.key))?;
    loop {
        let dto = receive(&mut session).await?.1;
        if let Control::ResultEnd { .. } = dto {
            break;
        }
    }
    decoder
        .finish(&data(&mut session, 8).await?)
        .map_err(HostFailure::from)?;
    assert!(matches!(
        terminal(&mut session, 2).await?,
        Control::Reply(Reply::Executed(_))
    ));
    session.send(0, &Control::Cancel { target: 2 })?;
    assert!(matches!(
        receive(&mut session).await?.1,
        Control::CancelAck {
            target: 2,
            requested: false
        }
    ));
    // Duplicate flow from a completed operation is harmless after the next query.
    assert_eq!(execute(&mut session, 3, "select 1").await?.rows, 1);
    session.send(0, &Control::Consumed(grant.key))?;
    session.send(0, &Control::Resume(grant))?;
    assert_eq!(execute(&mut session, 4, "select 1").await?.rows, 1);
    session.close().await
}
#[tokio::test]
async fn stop_drains_without_user_cancel_and_session_remains_usable() -> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let mut session = host(MemoryBudget::default()).await?;
    session.send(
        2,
        &Control::Execute {
            sql: "select 1".into(),
        },
    )?;
    let grant = ready_batch(&mut session).await?;
    session.send(0, &Control::Stop(grant.key))?;
    assert!(matches!(
        terminal(&mut session, 2).await?,
        Control::Reply(Reply::Executed(_))
    ));
    assert_eq!(execute(&mut session, 3, "select 1").await?.rows, 1);
    session.close().await
}
#[tokio::test]
#[allow(
    clippy::explicit_write,
    reason = "designated acceptance containment policy output matches the existing timing harness"
)]
async fn facet_placeholders_return_unsupported_and_not_advertised() -> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let mut session = host(MemoryBudget::default()).await?;
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        let _ = writeln!(
            std::io::stderr(),
            "Linux containment: {}",
            session.containment()
        );
    }
    for (index, dto) in [
        Control::Catalog {
            kind: openrows_host_protocol::CatalogKind::Databases,
            limit: 32,
        },
        Control::Editor,
        Control::Admin,
        Control::Bulk,
    ]
    .iter()
    .enumerate()
    {
        session.send(index as u64 + 2, dto)?;
        assert!(matches!(
            terminal(&mut session, index as u64 + 2).await?,
            Control::Error(WireError::Unsupported(_))
        ));
    }
    session.close().await
}

#[tokio::test]
async fn cancelling_a_queued_write_never_dispatches_it_to_sqlite() -> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let mut session = host(MemoryBudget::default()).await?;
    session.send(
        2,
        &Control::Execute {
            sql: "select 1".into(),
        },
    )?;
    let grant = ready_batch(&mut session).await?;
    session.send(
        3,
        &Control::Execute {
            sql: "create table should_not_exist (id integer)".into(),
        },
    )?;
    assert!(matches!(receive(&mut session).await?.1, Control::Accepted));
    session.send(0, &Control::Cancel { target: 3 })?;
    assert!(matches!(
        receive(&mut session).await?.1,
        Control::CancelAck {
            target: 3,
            requested: false
        }
    ));
    session.send(0, &Control::Stop(grant.key))?;
    assert!(matches!(
        terminal(&mut session, 2).await?,
        Control::Reply(Reply::Executed(_))
    ));
    assert!(
        matches!(terminal(&mut session, 3).await?, Control::Error(WireError::Server { code, .. }) if code == "57014")
    );
    session.send(
        4,
        &Control::Execute {
            sql: "select * from should_not_exist".into(),
        },
    )?;
    assert!(
        matches!(terminal(&mut session, 4).await?, Control::Error(WireError::Server { message, .. }) if message.contains("no such table"))
    );
    session.close().await
}
