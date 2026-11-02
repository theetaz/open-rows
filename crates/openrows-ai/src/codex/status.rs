// SPDX-License-Identifier: Apache-2.0
//! Explicit discovery and account states, read only through the CLI's own
//! `account/read`; credential files are never opened and email is never kept.
use super::{
    Bridge,
    process::Launch,
    rpc::Rpc,
    version::{self, Support, Version},
};
use crate::{AiError, Capabilities, Model, RequestOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::future::Future;

/// How the CLI reports its own sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AccountMode {
    /// Managed `ChatGPT` sign-in.
    Chatgpt,
    /// An API key stored by the CLI.
    ApiKey,
    /// Another account type this bridge does not interpret.
    Other,
}
/// One explicit state per discovery outcome; installed is not signed in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum CodexStatus {
    /// No executable at the configured path or in the probed locations.
    NotInstalled,
    /// Installed and supported, but the CLI reports no account.
    SignedOut {
        /// Reported CLI release.
        version: String,
        /// Later than the tested release.
        newer: bool,
    },
    /// Installed, supported and signed in through the CLI.
    SignedIn {
        /// Reported CLI release.
        version: String,
        /// Later than the tested release.
        newer: bool,
        /// Reported account type.
        mode: AccountMode,
        /// Reported plan, when the CLI knows it.
        plan: Option<String>,
    },
    /// Older, different-major or unrecognised release, or a missing protocol method.
    Unsupported {
        /// Reported release, when it could be read.
        version: Option<String>,
        /// The release this bridge was tested with.
        tested: String,
    },
    /// The probe failed with a fixed reason.
    Error {
        /// Scrubbed failure category.
        reason: AiError,
    },
}
fn unsupported(version: Option<Version>) -> CodexStatus {
    CodexStatus::Unsupported {
        version: version.map(|value| value.to_string()),
        tested: version::TESTED_VERSION.into(),
    }
}
/// Cancellation and the caller's total deadline around one probe.
pub(crate) async fn bounded<T>(
    options: &RequestOptions,
    work: impl Future<Output = Result<T, AiError>>,
) -> Result<T, AiError> {
    tokio::select! {
        biased;
        () = options.cancel.cancelled() => Err(AiError::Cancelled),
        result = tokio::time::timeout(options.timeout, work) => result.map_err(|_| AiError::Timeout)?,
    }
}
pub(crate) async fn probe(bridge: &Bridge, options: &RequestOptions) -> (CodexStatus, Vec<Model>) {
    let Ok(launch) = bridge.launch() else {
        return (CodexStatus::NotInstalled, Vec::new());
    };
    bounded(options, async { Ok(inspect(&launch).await) })
        .await
        .unwrap_or_else(|reason| (CodexStatus::Error { reason }, Vec::new()))
}
/// The version gate shared by status, sign-in and conversations.
pub(crate) async fn supported(launch: &Launch) -> Result<(Version, bool), CodexStatus> {
    let version = match version::probe(launch).await {
        Ok(Some(version)) => version,
        Ok(None) => return Err(unsupported(None)),
        Err(reason) => return Err(CodexStatus::Error { reason }),
    };
    match version.support() {
        Support::Unsupported => Err(unsupported(Some(version))),
        Support::Tested => Ok((version, false)),
        Support::Newer => Ok((version, true)),
    }
}
async fn inspect(launch: &Launch) -> (CodexStatus, Vec<Model>) {
    let (version, newer) = match supported(launch).await {
        Ok(found) => found,
        Err(status) => return (status, Vec::new()),
    };
    match account(launch, version, newer).await {
        Ok(found) => found,
        Err(AiError::Unsupported) => (unsupported(Some(version)), Vec::new()),
        Err(reason) => (CodexStatus::Error { reason }, Vec::new()),
    }
}
async fn account(
    launch: &Launch,
    version: Version,
    newer: bool,
) -> Result<(CodexStatus, Vec<Model>), AiError> {
    let mut rpc = Rpc::connect(launch, false).await?;
    let status = read(&mut rpc, version, newer).await;
    let models = match &status {
        Ok(CodexStatus::SignedIn { .. }) => models(&mut rpc).await,
        _ => Ok(Vec::new()),
    };
    rpc.close().await;
    Ok((status?, models?))
}
/// Re-read the account through the CLI; used after sign-in as well.
pub(crate) async fn read(
    rpc: &mut Rpc,
    version: Version,
    newer: bool,
) -> Result<CodexStatus, AiError> {
    let result = rpc
        .request("account/read", json!({"refreshToken": false}))
        .await?;
    let account = &result["account"];
    if account.is_null() {
        return Ok(CodexStatus::SignedOut {
            version: version.to_string(),
            newer,
        });
    }
    let mode = match account["type"].as_str() {
        Some("chatgpt") => AccountMode::Chatgpt,
        Some("apiKey") => AccountMode::ApiKey,
        Some(_) => AccountMode::Other,
        None => return Err(AiError::InvalidResponse),
    };
    let plan = account["planType"]
        .as_str()
        .filter(|plan| {
            *plan != "unknown"
                && plan.len() <= 48
                && plan.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        })
        .map(str::to_owned);
    Ok(CodexStatus::SignedIn {
        version: version.to_string(),
        newer,
        mode,
        plan,
    })
}
async fn models(rpc: &mut Rpc) -> Result<Vec<Model>, AiError> {
    let result = rpc
        .request("model/list", json!({"limit": 64, "includeHidden": false}))
        .await?;
    let data = result["data"].as_array().ok_or(AiError::InvalidResponse)?;
    Ok(data
        .iter()
        .take(64)
        .filter(|item| item["hidden"] != true)
        .filter_map(model)
        .collect())
}
fn model(value: &Value) -> Option<Model> {
    let bounded = |text: &str| !text.is_empty() && text.len() <= 256;
    let id = value["model"].as_str().filter(|id| bounded(id))?;
    let name = value["displayName"]
        .as_str()
        .filter(|name| bounded(name))
        .unwrap_or(id);
    let vision = value["inputModalities"]
        .as_array()
        .map(|items| items.iter().any(|item| item == "image"));
    Some(Model {
        id: id.into(),
        name: name.into(),
        capabilities: Capabilities {
            streaming: Some(true),
            // Application tools are offered as app-server dynamic tools.
            tool_calling: Some(true),
            vision,
            ..Capabilities::default()
        },
    })
}
