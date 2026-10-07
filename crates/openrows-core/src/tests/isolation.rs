// SPDX-License-Identifier: Apache-2.0
//! A panicking driver fails only its own session (NFR-REL-02).
use super::fake::{Fake, panic_now};
use super::{TestResult, last_event, manager, open};
use crate::{ErrorKind, QueryEvent, SessionState};

#[tokio::test]
async fn driver_panic_fails_only_its_session() -> TestResult {
    let (manager, _store) = manager();
    let mut changes = manager.subscribe();
    let healthy = open(&manager, None).await?;
    let doomed = open(&manager, None).await?;
    let mut query = manager.execute(doomed, "panic")?;
    let QueryEvent::Failed { error } = last_event(&mut query).await? else {
        return Err("expected Failed".into());
    };
    assert_eq!(error.kind, ErrorKind::Internal);
    assert!(
        error.message.contains("fake driver bug"),
        "{}",
        error.message
    );
    assert_eq!(manager.state(doomed), Some(SessionState::Failed));
    let failed = loop {
        let change = changes.recv().await?;
        if change.state == SessionState::Failed {
            break change;
        }
    };
    assert_eq!(failed.session, doomed);
    assert_eq!(failed.error.map(|e| e.kind), Some(ErrorKind::Internal));

    assert_eq!(manager.state(healthy), Some(SessionState::Idle));
    let mut next = manager.execute(healthy, "rows 1 1")?;
    assert!(matches!(
        last_event(&mut next).await?,
        QueryEvent::Completed { .. }
    ));
    let fresh = open(&manager, None).await?;
    assert_eq!(manager.state(fresh), Some(SessionState::Idle));
    manager.close(doomed).await?;
    Ok(())
}

#[tokio::test]
async fn panic_while_connecting_is_an_internal_error() -> TestResult {
    let (manager, _store) = manager();
    let error = manager
        .open(async {
            panic_now();
            Ok(Fake::boxed(None))
        })
        .await
        .err()
        .ok_or("connected")?;
    assert_eq!(error.kind, ErrorKind::Internal);
    Ok(())
}
