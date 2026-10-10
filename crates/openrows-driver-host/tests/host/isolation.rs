// SPDX-License-Identifier: Apache-2.0
//! Real disposable-child failures never damage an independently connected host.
use crate::support;
use openrows_host_client::{CLOSE_GRACE, HostFailure, HostSession, HostSupervisor, MemoryBudget};
use openrows_host_protocol::{Control, Reply, WireError};
use std::time::{Duration, Instant};
use support::*;

#[tokio::test]
async fn native_abort_segfault_and_rust_panic_reap_only_the_identified_child()
-> Result<(), HostFailure> {
    let _permit = host_test().await?;
    for hook in [
        "OPENROWS_TEST_ABORT",
        "OPENROWS_TEST_SEGFAULT",
        "OPENROWS_TEST_PANIC",
        "OPENROWS_TEST_FATAL_ALLOC_FAILURE",
    ] {
        let memory = MemoryBudget::default();
        let mut survivor = host(memory.clone()).await?;
        let mut victim = host(memory.clone()).await?;
        let pid = victim.pid();
        let directory = victim.directory().to_path_buf();
        victim.send(2, &Control::Execute { sql: hook.into() })?;
        assert_eq!(failed(&victim).await, HostFailure::Exited);
        assert_reaped(pid, &directory);
        assert_eq!(execute(&mut survivor, 2, "select 1").await?.rows, 1);
        survivor.close().await?;
        assert_eq!(memory.reserved(), 0);
    }
    Ok(())
}
#[tokio::test]
async fn allocation_error_preserves_connection_and_controlled_growth_hits_memory_limit()
-> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let memory = MemoryBudget::default();
    let mut survivor = host(memory.clone()).await?;
    let mut victim = host(memory.clone()).await?;
    victim.send(
        2,
        &Control::Execute {
            sql: "OPENROWS_TEST_ALLOC_FAILURE".into(),
        },
    )?;
    assert!(
        matches!(terminal(&mut victim, 2).await?, Control::Error(WireError::Server { code, .. }) if code == "53200")
    );
    assert_eq!(execute(&mut victim, 3, "select 1").await?.rows, 1);
    let pid = victim.pid();
    let directory = victim.directory().to_path_buf();
    victim.send(
        4,
        &Control::Execute {
            sql: "OPENROWS_TEST_MEMORY".into(),
        },
    )?;
    assert_eq!(failed(&victim).await, HostFailure::MemoryLimit);
    assert_reaped(pid, &directory);
    assert_eq!(execute(&mut survivor, 2, "select 1").await?.rows, 1);
    survivor.close().await?;
    assert_eq!(memory.reserved(), 0);
    Ok(())
}
#[tokio::test]
async fn missing_heartbeats_probe_then_terminate_and_reap() -> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let mut session = host(MemoryBudget::default()).await?;
    let pid = session.pid();
    let directory = session.directory().to_path_buf();
    session.send(
        2,
        &Control::Execute {
            sql: "OPENROWS_TEST_NO_HEARTBEAT".into(),
        },
    )?;
    assert!(matches!(
        terminal(&mut session, 2).await?,
        Control::Reply(Reply::Pong)
    ));
    assert_eq!(failed(&session).await, HostFailure::Unresponsive);
    assert_reaped(pid, &directory);
    Ok(())
}
#[tokio::test]
async fn close_and_cancel_kill_a_noncooperative_worker_after_grace() -> Result<(), HostFailure> {
    let _permit = host_test().await?;
    for close in [false, true] {
        let session = hanging_host().await?;
        let started = Instant::now();
        // Two seconds of bounded scheduler/reap slack, independent of heartbeats.
        tokio::time::timeout(
            CLOSE_GRACE + Duration::from_secs(2),
            terminate_hung(session, close),
        )
        .await
        .map_err(|_| HostFailure::Unresponsive)??;
        assert!(started.elapsed() >= CLOSE_GRACE);
    }
    Ok(())
}

async fn hanging_host() -> Result<HostSession, HostFailure> {
    let mut session = host(MemoryBudget::default()).await?;
    session.send(
        2,
        &Control::Execute {
            sql: "OPENROWS_TEST_HANG_NOTICE".into(),
        },
    )?;
    assert!(matches!(receive(&mut session).await?.1, Control::Accepted));
    // Only the worker emits this notice, after checking queued cancellation.
    let (header, notice) = receive(&mut session).await?;
    assert_eq!(header.request, 2);
    assert!(
        matches!(notice, Control::Notice { message, .. } if message == "test-only worker started")
    );
    Ok(session)
}

async fn terminate_hung(mut session: HostSession, close: bool) -> Result<(), HostFailure> {
    let pid = session.pid();
    let directory = session.directory().to_path_buf();
    if close {
        assert!(matches!(
            session.close().await,
            Err(HostFailure::Unresponsive | HostFailure::Exited)
        ));
    } else {
        session.send(0, &Control::Cancel { target: 2 })?;
        assert!(matches!(
            receive(&mut session).await?.1,
            Control::CancelAck {
                target: 2,
                requested: true
            }
        ));
        assert_eq!(failed(&session).await, HostFailure::Unresponsive);
    }
    assert_reaped(pid, &directory);
    Ok(())
}
#[tokio::test]
async fn circuit_refuses_fourth_real_crash_without_spawning() -> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let options = options(MemoryBudget::default());
    let supervisor = HostSupervisor::new(options.clone());
    for _ in 0..3 {
        let mut session = supervisor.spawn().await?;
        session.send(1, &Control::Connect(config()))?;
        terminal(&mut session, 1).await?;
        session.send(
            2,
            &Control::Execute {
                sql: "OPENROWS_TEST_ABORT".into(),
            },
        )?;
        assert_eq!(failed(&session).await, HostFailure::Exited);
    }
    assert!(matches!(supervisor.spawn().await, Err(HostFailure::Spawn)));
    assert_eq!(options.memory.reserved(), 0);
    Ok(())
}
#[tokio::test]
async fn drop_initiates_nonblocking_targeted_reap_and_cleanup() -> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let session = host(MemoryBudget::default()).await?;
    let pid = session.pid();
    let directory = session.directory().to_path_buf();
    let mut state = session.subscribe();
    drop(session);
    state.changed().await.map_err(|_| HostFailure::Exited)?;
    assert_reaped(pid, &directory);
    Ok(())
}

#[tokio::test]
async fn host_failure_releases_exclusive_credit_before_publishing_failure()
-> Result<(), HostFailure> {
    let _permit = host_test().await?;
    let memory = MemoryBudget::default();
    let mut session = host(memory.clone()).await?;
    let pid = session.pid();
    let directory = session.directory().to_path_buf();
    session.send(
        2,
        &Control::Execute {
            sql: "select hex(zeroblob(2200000))".into(),
        },
    )?;
    loop {
        if let Control::BatchReady(grant) = receive(&mut session).await?.1 {
            assert!(grant.exclusive);
            session.send(0, &Control::Grant(grant))?;
            assert!(memory.reserved() > openrows_host_client::HOST_MEMORY);
            break;
        }
    }
    session.poison(HostFailure::Protocol);
    assert_eq!(failed(&session).await, HostFailure::Protocol);
    assert_eq!(memory.reserved(), 0);
    assert_reaped(pid, &directory);
    Ok(())
}
