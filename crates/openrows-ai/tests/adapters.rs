// SPDX-License-Identifier: Apache-2.0
//! Each protocol runs the same mandatory matrix against real loopback sockets.
mod support;
use openrows_ai::{AiError, Health, Provider, ProviderKind, RequestOptions, TextRequest};
use support::{Mock, TestResult, enabled, models, response};
use tokio::sync::mpsc;

async fn success(kind: ProviderKind) -> TestResult {
    let mut replies = vec![models(kind)];
    if kind == ProviderKind::OpenRouter {
        replies.insert(
            0,
            response(
                200,
                r#"{"data":{"usage":2,"limit":10,"limit_remaining":8}}"#,
            ),
        );
    }
    let mut mock = Mock::new(replies).await?;
    let result = mock
        .provider(kind, enabled())?
        .verify(&RequestOptions::default())
        .await?;
    assert_eq!(result.health, Health::Verified);
    assert_eq!(result.models.len(), 1);
    let request = mock.requests.recv().await.ok_or("no request")?;
    assert!(request.starts_with("GET "));
    assert!(!request.contains("messages") && !request.contains("contents"));
    if kind == ProviderKind::OpenRouter {
        assert_eq!(result.usage.ok_or("usage missing")?.remaining, Some(8.0));
    }
    Ok(())
}
async fn streaming(kind: ProviderKind) -> TestResult {
    let mut mock = Mock::new(vec![support::stream(kind)]).await?;
    let (sink, mut deltas) = mpsc::channel(2);
    let request = TextRequest {
        model: "model".into(),
        text: "hello".into(),
        max_output: 12,
    };
    mock.provider(kind, enabled())?
        .stream(&request, sink, &RequestOptions::default())
        .await?;
    assert_eq!(deltas.recv().await.as_deref(), Some("Hello"));
    assert!(deltas.recv().await.is_none());
    let wire = mock.requests.recv().await.ok_or("request missing")?;
    assert!(wire.starts_with("POST "));
    if kind == ProviderKind::OpenAi {
        assert!(wire.contains("\"store\":false"));
        assert!(wire.starts_with("POST /responses "));
    }
    if kind == ProviderKind::Gemini {
        assert!(wire.contains("x-goog-api-key: test-only"));
        assert!(!wire.contains("?key="));
    }
    Ok(())
}
async fn failure(kind: ProviderKind, status: u16, expected: AiError) -> TestResult {
    let mock = Mock::new(vec![response(status, "test-only")]).await?;
    let error = mock
        .provider(kind, enabled())?
        .verify(&RequestOptions::default())
        .await
        .err()
        .ok_or("expected failure")?;
    assert_eq!(error, expected);
    assert!(!format!("{error:?} {error}").contains("test-only"));
    Ok(())
}
async fn malformed(kind: ProviderKind) -> TestResult {
    failure(kind, 200, AiError::InvalidResponse).await?;
    let mock = Mock::new(vec![response(200, "data: {}\n\n")]).await?;
    let (sink, _receiver) = mpsc::channel(1);
    let request = TextRequest {
        model: "model".into(),
        text: "hello".into(),
        max_output: 10,
    };
    assert_eq!(
        mock.provider(kind, enabled())?
            .stream(&request, sink, &RequestOptions::default())
            .await,
        Err(AiError::InvalidResponse)
    );
    Ok(())
}
async fn cancelled(kind: ProviderKind) -> TestResult {
    let mut mock = Mock::new(vec!["hang".into()]).await?;
    let provider = mock.provider(kind, enabled())?;
    let options = RequestOptions::default();
    let cancel = async {
        mock.requests.recv().await;
        options.cancel.cancel();
    };
    let (result, ()) = tokio::join!(provider.verify(&options), cancel);
    assert_eq!(result, Err(AiError::Cancelled));
    Ok(())
}
async fn timeout(kind: ProviderKind) -> TestResult {
    let mock = Mock::new(vec!["hang".into()]).await?;
    let options = RequestOptions {
        timeout: std::time::Duration::from_millis(30),
        ..RequestOptions::default()
    };
    assert_eq!(
        mock.provider(kind, enabled())?.verify(&options).await,
        Err(AiError::Timeout)
    );
    Ok(())
}
macro_rules! adapter {
    ($name:ident, $kind:ident) => {
        mod $name {
            use super::*;
            const KIND: ProviderKind = ProviderKind::$kind;
            #[tokio::test]
            async fn models_success() -> TestResult {
                success(KIND).await
            }
            #[tokio::test]
            async fn streaming_deltas() -> TestResult {
                streaming(KIND).await
            }
            #[tokio::test]
            async fn auth_failure() -> TestResult {
                failure(KIND, 401, AiError::Auth).await
            }
            #[tokio::test]
            async fn rate_limit_retry_after() -> TestResult {
                failure(
                    KIND,
                    429,
                    AiError::RateLimit {
                        retry_after: Some(7),
                    },
                )
                .await
            }
            #[tokio::test]
            async fn malformed_response() -> TestResult {
                malformed(KIND).await
            }
            #[tokio::test]
            async fn cancellation() -> TestResult {
                cancelled(KIND).await
            }
            #[tokio::test]
            async fn total_timeout() -> TestResult {
                timeout(KIND).await
            }
        }
    };
}
adapter!(anthropic, Anthropic);
adapter!(openai, OpenAi);
adapter!(openrouter, OpenRouter);
adapter!(gemini, Gemini);
adapter!(compatible, Compatible);
adapter!(ollama, Ollama);
adapter!(lm_studio, LocalCompatible);
