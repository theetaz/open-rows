// SPDX-License-Identifier: Apache-2.0
//! The CLI's own managed `ChatGPT` sign-in; tokens stay with the CLI.
use super::{
    Bridge,
    process::Launch,
    rpc::{Inbound, Rpc},
    session::refuse,
    status::{CodexStatus, bounded, read, supported},
};
use crate::{AiError, RequestOptions};
use serde_json::json;
use tokio::sync::oneshot;
use url::Url;

pub(crate) async fn login(
    bridge: &Bridge,
    options: &RequestOptions,
    url: oneshot::Sender<String>,
) -> Result<CodexStatus, AiError> {
    let launch = bridge.launch()?;
    bounded(options, flow(&launch, url)).await
}
async fn flow(launch: &Launch, url: oneshot::Sender<String>) -> Result<CodexStatus, AiError> {
    let (version, newer) = supported(launch).await.map_err(|_| AiError::Unsupported)?;
    let mut rpc = Rpc::connect(launch, false).await?;
    let current = read(&mut rpc, version, newer).await?;
    if matches!(current, CodexStatus::SignedIn { .. }) {
        rpc.close().await;
        return Ok(current);
    }
    let started = rpc
        .request("account/login/start", json!({"type": "chatgpt"}))
        .await?;
    let id = started["loginId"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .ok_or(AiError::InvalidResponse)?
        .to_owned();
    let address = sign_in_url(started["authUrl"].as_str().unwrap_or_default())?;
    // The receiver may already be gone; the CLI's local callback still completes.
    let _ = url.send(address);
    completion(&mut rpc, &id).await?;
    let status = read(&mut rpc, version, newer).await;
    rpc.close().await;
    status
}
/// Only an HTTPS page on an `OpenAI` or `ChatGPT` host is opened in the browser.
fn sign_in_url(text: &str) -> Result<String, AiError> {
    let url = Url::parse(text).map_err(|_| AiError::InvalidResponse)?;
    let host = url.host_str().unwrap_or_default();
    let trusted = ["openai.com", "chatgpt.com"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")));
    if text.len() > 4096 || url.scheme() != "https" || !trusted || !url.username().is_empty() {
        return Err(AiError::InvalidResponse);
    }
    Ok(url.into())
}
async fn completion(rpc: &mut Rpc, id: &str) -> Result<(), AiError> {
    loop {
        match rpc.next().await? {
            Inbound::Notification { method, params }
                if method == "account/login/completed" && params["loginId"] == id =>
            {
                return if params["success"] == true {
                    Ok(())
                } else {
                    Err(AiError::Auth)
                };
            }
            Inbound::Notification { .. } => {}
            Inbound::Request { id, method, .. } => refuse(rpc, id, &method).await?,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::sign_in_url;
    #[test]
    fn only_https_openai_sign_in_pages_are_opened() {
        assert!(sign_in_url("https://auth.openai.com/oauth/authorize?x=1").is_ok());
        assert!(sign_in_url("https://chatgpt.com/auth").is_ok());
        assert!(sign_in_url("http://auth.openai.com/").is_err());
        assert!(sign_in_url("https://openai.com.example.net/").is_err());
        assert!(sign_in_url("file:///etc/passwd").is_err());
        assert!(sign_in_url("https://user@auth.openai.com/").is_err());
    }
}
