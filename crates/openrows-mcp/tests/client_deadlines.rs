// SPDX-License-Identifier: Apache-2.0
//! In-flight HTTP cancellation and deadline ownership.
use openrows_ai::Cancellation;
use openrows_mcp::client::{ClientError, Credentials, Endpoint, ServerConfig, discover};
type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
fn config(endpoint: Endpoint) -> ServerConfig {
    ServerConfig {
        id: "fixture".into(),
        name: "Fixture".into(),
        endpoint,
        enabled_tools: vec![],
    }
}
#[tokio::test]
async fn cancellation_interrupts_http_initialization() -> TestResult {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await?;
        let _ = entered.send(());
        std::future::pending::<()>().await;
        Ok::<(), std::io::Error>(())
    });
    let cancel = Cancellation::default();
    let other = cancel.clone();
    let client = tokio::spawn(async move {
        discover(
            &config(Endpoint::Http {
                url: format!("http://127.0.0.1:{port}/mcp"),
            }),
            &Credentials::default(),
            &other,
        )
        .await
    });
    waiting.await?;
    cancel.cancel();
    assert!(matches!(client.await?, Err(ClientError::Cancelled)));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    Ok(())
}

#[tokio::test]
async fn initialize_deadline_closes_a_stalled_http_exchange() -> TestResult {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await?;
        std::future::pending::<()>().await;
        Ok::<(), std::io::Error>(())
    });
    let result = discover(
        &config(Endpoint::Http {
            url: format!("http://127.0.0.1:{port}/mcp"),
        }),
        &Credentials::default(),
        &Cancellation::default(),
    )
    .await;
    // The outer deadline or reqwest's equal transport deadline can win the race.
    assert!(matches!(
        result,
        Err(ClientError::Timeout | ClientError::Transport)
    ));
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    Ok(())
}
