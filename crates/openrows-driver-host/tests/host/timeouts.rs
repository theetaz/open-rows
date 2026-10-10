// SPDX-License-Identifier: Apache-2.0
//! A configured timeout stays distinct while forced host loss fails queued callers.
use crate::{core, support};
use openrows_core::{ErrorKind, QueryEvent, SessionManager};
use openrows_driver_api::Session;
use std::{sync::Arc, time::Duration};
#[tokio::test]
async fn worker_timeout_reports_timeout_but_queued_callers_and_session_report_host_loss() {
    let _permit = support::host_test().await.unwrap();
    let manager = SessionManager::new(Arc::new(openrows_results::ResultStore::default()));
    let mut changes = manager.subscribe();
    let connection = core::hosted().await;
    let session = manager
        .open(async move { Ok(Box::new(connection) as Box<dyn Session>) })
        .await
        .unwrap();
    manager
        .set_statement_timeout(session, Some(Duration::from_millis(200)))
        .unwrap();
    let mut active = manager
        .execute(session, "OPENROWS_TEST_HANG_NOTICE")
        .unwrap();
    assert!(matches!(
        active.next_event().await,
        Some(QueryEvent::Started { .. })
    ));
    assert!(matches!(
        active.next_event().await,
        Some(QueryEvent::Notice { .. })
    ));
    let queued = manager.execute(session, "SELECT 2").unwrap();
    let outcome = core::events(active).await;
    assert_eq!(outcome.iter().filter(|event| matches!(event, QueryEvent::Failed { error } if error.kind == ErrorKind::Timeout)).count(), 1);
    let queued = core::events(queued).await;
    assert_eq!(queued.iter().filter(|event| matches!(event, QueryEvent::Failed { error } if error.kind == ErrorKind::DriverHost)).count(), 1);
    let failure = tokio::time::timeout(Duration::from_secs(4), failed(&mut changes, session))
        .await
        .unwrap();
    assert_eq!(failure.error.unwrap().kind, ErrorKind::DriverHost);
    manager.close(session).await.unwrap();
}

async fn failed(
    changes: &mut tokio::sync::broadcast::Receiver<openrows_core::SessionEvent>,
    session: openrows_types::SessionId,
) -> openrows_core::SessionEvent {
    loop {
        let event = changes.recv().await.unwrap();
        if event.session == session && event.state == openrows_core::SessionState::Failed {
            return event;
        }
    }
}
