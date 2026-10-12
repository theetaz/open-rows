// SPDX-License-Identifier: Apache-2.0
//! Facet calls queue on the session's own connection.
use super::{TestResult, manager, open};
use openrows_driver_api::DriverError;

#[tokio::test]
async fn facet_calls_need_a_connection_and_an_open_session() -> TestResult {
    let (manager, _) = manager();
    let session = open(&manager, None).await?;
    let port = manager.facet_port(session)?;
    let refused = port.call(|_| Box::pin(async { Ok(()) })).await;
    assert!(
        matches!(&refused, Err(DriverError::Unsupported(message)) if message.contains("no catalog")),
        "{refused:?}"
    );
    manager.close(session).await?;
    let closed = port.call(|_| Box::pin(async { Ok(()) })).await;
    assert!(
        matches!(&closed, Err(DriverError::Unsupported(message)) if message.contains("reconnect")),
        "{closed:?}"
    );
    assert!(manager.facet_port(session).is_err());
    Ok(())
}
