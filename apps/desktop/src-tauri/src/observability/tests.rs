// SPDX-License-Identifier: Apache-2.0
//! Proofs that the log output never holds a password. These run the real JSON
//! subscriber and the real daily file over the real scrubbing writer. The
//! query-text proof lives with the query runner (`tests/observability.rs` in
//! `openrows-core`).
use std::fs;

use tracing::level_filters::LevelFilter;
use tracing::subscriber::with_default;

use super::panic::log_panic;
use super::sink::{self, Shared, subscriber};

/// Run `emit` under a TRACE-level JSON subscriber; return the log text.
fn capture(emit: impl FnOnce()) -> String {
    let output = Shared::new(Vec::<u8>::new());
    with_default(subscriber(output.clone(), LevelFilter::TRACE), emit);
    output.text()
}

const PASSWORD: &str = "hunter2-pass";

#[test]
fn passwords_in_urls_fields_and_text_never_reach_the_log() {
    let text = capture(|| {
        tracing::warn!(
            url = "postgres://app:hunter2-pass@db.internal:5432/orders?sslmode=require",
            "connection test failed"
        );
        tracing::warn!(password = PASSWORD, "credentials rejected");
        tracing::info!(detail = "server said: password=hunter2-pass", "auth error");
    });
    assert!(!text.contains(PASSWORD), "password leaked: {text}");
    assert!(text.contains("connection test failed"), "{text}");
    assert!(text.contains("[redacted]"), "{text}");
    for line in text.lines() {
        let parsed = serde_json::from_str::<serde_json::Value>(line);
        assert!(parsed.is_ok(), "log line is not JSON: {line}");
    }
}

#[test]
fn levels_below_the_configured_level_are_not_written() {
    let output = Shared::new(Vec::<u8>::new());
    with_default(subscriber(output.clone(), LevelFilter::INFO), || {
        tracing::debug!("debug detail");
        tracing::info!("kept");
    });
    let text = output.text();
    assert!(!text.contains("debug detail"), "{text}");
    assert!(text.contains("kept"), "{text}");
}

#[test]
fn a_panic_is_logged_with_its_location_and_without_its_secret() {
    let output = Shared::new(Vec::<u8>::new());
    with_default(subscriber(output.clone(), LevelFilter::TRACE), || {
        log_panic(
            "src/connections/sessions.rs:42",
            "login failed, password=hunter2-pass",
            "frame 0",
        );
    });
    let text = output.text();
    assert!(!text.contains(PASSWORD), "{text}");
    assert!(text.contains("src/connections/sessions.rs:42"), "{text}");
    assert!(text.contains("\"level\":\"ERROR\""), "{text}");
}

#[test]
fn the_daily_file_is_named_for_the_app_and_holds_scrubbed_lines()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let file = sink::rolling(dir.path(), LevelFilter::INFO)?;
    with_default(file, || {
        tracing::warn!(password = PASSWORD, "credentials rejected");
    });
    let mut names = fs::read_dir(dir.path())?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    names.retain(|name| {
        name.starts_with("openrows.")
            && std::path::Path::new(name)
                .extension()
                .is_some_and(|extension| extension == "log")
    });
    assert_eq!(names.len(), 1, "{names:?}");
    let text = fs::read_to_string(dir.path().join(&names[0]))?;
    assert!(text.contains("credentials rejected"), "{text}");
    assert!(!text.contains(PASSWORD), "{text}");
    Ok(())
}

#[test]
fn ai_keys_headers_and_crash_reports_are_scrubbed() {
    let secret = openrows_secrets::SecretString::from("test-only");
    let text = capture(|| {
        tracing::warn!(api_key = secret.expose(), error = %openrows_ai::AiError::Auth, "AI verification failed");
        tracing::info!(header = "x-api-key: test-only", "AI header rejected");
        tracing::info!(header = "x-goog-api-key: test-only", "AI header rejected");
        tracing::debug!(credential = ?secret, "AI credential type");
        log_panic("ai.rs:1", "api_key=test-only", "frame 0");
    });
    assert!(!text.contains(secret.expose()), "AI key leaked");
    assert!(text.contains("[redacted]"));
    assert!(text.contains("Provider authentication failed"));
}

#[test]
fn external_protocol_and_http_diagnostics_cannot_log_payloads_or_headers() {
    let text = capture(|| {
        tracing::error!(target: "rmcp::transport", payload = "synthetic-mcp-value", "raw response");
        tracing::warn!(target: "reqwest::connect", headers = "synthetic-header-value", "raw headers");
        tracing::debug!(target: "hyper::proto", body = "synthetic-body-value", "raw body");
        tracing::trace!(target: "h2::codec", frame = "synthetic-frame-value", "raw frame");
        tracing::info!("kept application event");
    });
    assert!(!text.contains("synthetic"));
    assert!(text.contains("kept application event"));
}
