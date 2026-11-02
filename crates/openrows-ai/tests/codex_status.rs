// SPDX-License-Identifier: Apache-2.0
//! Codex discovery, version, account and sign-in states against the fake app-server.
mod codex_support;
use codex_support::*;
use openrows_ai::{
    AiError,
    codex::{AccountMode, CodexStatus, TESTED_VERSION},
};
use serde_json::json;
use std::time::Duration;

const PROBE: Duration = Duration::from_secs(20);

#[tokio::test]
async fn a_missing_executable_is_not_installed() -> Result {
    let fixture = Fixture::with_executable("", "/nonexistent/openrows-test/codex")?;
    let (status, models) = fixture.bridge.status(&options(PROBE)).await;
    assert_eq!(status, CodexStatus::NotInstalled);
    assert!(models.is_empty());
    Ok(())
}
#[tokio::test]
async fn signed_in_reports_mode_plan_and_visible_models_but_never_email() -> Result {
    let fixture = Fixture::new("signed-in")?;
    let (status, models) = fixture.bridge.status(&options(PROBE)).await;
    assert_eq!(
        status,
        CodexStatus::SignedIn {
            version: TESTED_VERSION.into(),
            newer: false,
            mode: AccountMode::Chatgpt,
            plan: Some("plus".into()),
        }
    );
    assert!(!serde_json::to_string(&status)?.contains("example.com"));
    let ids: Vec<_> = models.iter().map(|model| model.id.as_str()).collect();
    assert_eq!(ids, ["gpt-fake"]);
    assert_eq!(models[0].capabilities.tool_calling, Some(true));
    assert_eq!(models[0].capabilities.vision, Some(true));
    Ok(())
}
#[tokio::test]
async fn signed_out_is_separate_from_installed() -> Result {
    let fixture = Fixture::new("signed-out")?;
    let (status, models) = fixture.bridge.status(&options(PROBE)).await;
    assert_eq!(
        status,
        CodexStatus::SignedOut {
            version: TESTED_VERSION.into(),
            newer: false
        }
    );
    assert!(models.is_empty());
    Ok(())
}
#[tokio::test]
async fn older_unknown_and_newer_releases_are_classified_before_launch() -> Result {
    let fixture = Fixture::new("signed-in")?;
    fixture.version("codex-cli 0.120.0")?;
    let (status, _) = fixture.bridge.status(&options(PROBE)).await;
    assert_eq!(
        status,
        CodexStatus::Unsupported {
            version: Some("0.120.0".into()),
            tested: TESTED_VERSION.into()
        }
    );
    assert!(
        !fixture.launched(),
        "app-server must not start for an old release"
    );
    fixture.version("codex-cli unknown")?;
    let (status, _) = fixture.bridge.status(&options(PROBE)).await;
    assert!(matches!(
        status,
        CodexStatus::Unsupported { version: None, .. }
    ));
    fixture.version("codex-cli 0.170.1")?;
    let (status, _) = fixture.bridge.status(&options(PROBE)).await;
    assert!(
        matches!(status, CodexStatus::SignedIn { newer: true, ref version, .. } if version == "0.170.1")
    );
    Ok(())
}
#[tokio::test]
async fn the_child_is_isolated_and_maximally_restricted() -> Result {
    let fixture = Fixture::new("signed-in")?;
    fixture.bridge.status(&options(PROBE)).await;
    let launch = fixture.recorded("launch")?;
    assert_eq!(
        launch["environment"],
        json!(["CODEX_HOME", "HOME", "LANG", "PATH", "TMPDIR"])
    );
    let root = canonical(&fixture.root());
    assert_eq!(
        canonical(launch["home"].as_str().unwrap_or_default().as_ref()),
        root.join("home")
    );
    assert_eq!(
        canonical(launch["cwd"].as_str().unwrap_or_default().as_ref()),
        root.join("work")
    );
    let args = launch["args"].as_array().cloned().unwrap_or_default();
    for setting in [
        "approval_policy=\"on-request\"",
        "sandbox_mode=\"read-only\"",
        "web_search=\"disabled\"",
        "features.shell_tool=false",
        "features.unified_exec=false",
        "mcp_servers={}",
    ] {
        assert!(args.contains(&json!(setting)), "{setting}");
    }
    let initialize = fixture.recorded("initialize")?;
    assert_eq!(initialize["clientInfo"]["name"], "openrows");
    assert_eq!(initialize["capabilities"]["experimentalApi"], false);
    Ok(())
}
#[tokio::test]
async fn sign_in_uses_the_cli_flow_and_rereads_the_account() -> Result {
    let fixture = Fixture::new("signed-out")?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let status = fixture.bridge.login(&options(PROBE), send).await?;
    assert_eq!(
        receive.await?,
        "https://auth.openai.com/oauth/authorize?state=fake"
    );
    assert!(matches!(
        status,
        CodexStatus::SignedIn {
            mode: AccountMode::Chatgpt,
            ..
        }
    ));
    Ok(())
}
#[tokio::test]
async fn an_untrusted_sign_in_url_is_never_released() -> Result {
    let fixture = Fixture::new("bad-url")?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let result = fixture.bridge.login(&options(PROBE), send).await;
    assert_eq!(result, Err(AiError::InvalidResponse));
    assert!(receive.await.is_err());
    Ok(())
}
#[tokio::test]
async fn cancellation_stops_a_probe() -> Result {
    let fixture = Fixture::new("signed-in")?;
    let options = options(PROBE);
    options.cancel.cancel();
    let (status, _) = fixture.bridge.status(&options).await;
    assert_eq!(
        status,
        CodexStatus::Error {
            reason: AiError::Cancelled
        }
    );
    Ok(())
}
#[test]
fn codex_settings_hold_no_key_endpoint_or_remote_opt_in() {
    let valid = config("");
    assert!(valid.validate().is_ok());
    assert!(config("/usr/local/bin/codex").validate().is_ok());
    for invalid in [
        config("relative/codex"),
        openrows_ai::ProviderConfig {
            secret_ref: "ai/fixture/api-key".into(),
            ..valid.clone()
        },
        openrows_ai::ProviderConfig {
            allow_insecure_remote: true,
            ..valid.clone()
        },
        openrows_ai::ProviderConfig {
            base_url: "/bin/codex\n".into(),
            ..valid.clone()
        },
    ] {
        assert_eq!(invalid.validate(), Err(AiError::InvalidConfig));
    }
}
