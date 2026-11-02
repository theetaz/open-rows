// SPDX-License-Identifier: Apache-2.0
//! Explicit opt-in smoke against the installed Codex CLI: discovery and account
//! status only. It never starts a thread, sends a prompt or uses quota.
use openrows_ai::{ProviderConfig, ProviderKind, RequestOptions, codex::Bridge};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires OPENROWS_CODEX_LIVE=1 and an installed codex CLI"]
async fn installed_codex_status_smoke() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("OPENROWS_CODEX_LIVE").as_deref() != Ok("1") {
        return Err("Set OPENROWS_CODEX_LIVE=1 explicitly".into());
    }
    let config = ProviderConfig {
        id: "live".into(),
        name: "Codex".into(),
        kind: ProviderKind::Codex,
        base_url: std::env::var("OPENROWS_CODEX_PATH").unwrap_or_default(),
        allow_insecure_remote: false,
        secret_ref: String::new(),
    };
    // A fresh dedicated root, exactly as the app uses under its data directory.
    let root = tempfile::tempdir()?;
    let bridge = Bridge::new(&config, root.path().join("codex"))?;
    let options = RequestOptions {
        timeout: Duration::from_secs(30),
        ..RequestOptions::default()
    };
    let (status, models) = bridge.status(&options).await;
    let summary = serde_json::json!({"status": status, "models": models.len()});
    std::io::Write::write_all(&mut std::io::stderr(), format!("{summary}\n").as_bytes())?;
    Ok(())
}
