// SPDX-License-Identifier: Apache-2.0
//! Official rmcp framing with no inherited credentials, redirects or protocol logs.
use super::{ClientError, Credentials, Endpoint, ServerConfig};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::{
    RoleClient, ServiceExt, service::RunningService, transport::StreamableHttpClientTransport,
};
use std::{collections::HashMap, process::Stdio, time::Duration};

pub(super) struct Session {
    pub service: RunningService<RoleClient, ()>,
    _child: ChildGuard,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.service.cancellation_token().cancel();
    }
}
// Own cleanup before awaiting initialization, so failed handshakes and cancelled
// connects kill and reap the process just like established sessions do.
struct ChildGuard(Option<tokio::process::Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            tokio::spawn(async move {
                let _ = child.kill().await;
                let _ = child.wait().await;
            });
        }
    }
}
pub(super) async fn connect(
    config: &ServerConfig,
    secrets: &Credentials,
) -> Result<Session, ClientError> {
    config.validate()?;
    secrets.validate()?;
    match &config.endpoint {
        Endpoint::Stdio { command, args } => stdio(command, args, secrets).await,
        Endpoint::Http { url } => http(url, secrets).await,
    }
}
async fn stdio(
    executable: &str,
    args: &[String],
    secrets: &Credentials,
) -> Result<Session, ClientError> {
    let mut command = stdio_command(executable, args, secrets);
    let mut guard = ChildGuard(Some(command.spawn().map_err(|_| ClientError::Transport)?));
    let child = guard.0.as_mut().ok_or(ClientError::Transport)?;
    let stdout = child.stdout.take().ok_or(ClientError::Transport)?;
    let stdin = child.stdin.take().ok_or(ClientError::Transport)?;
    let service = ().serve((stdout, stdin)).await.map_err(|_| ClientError::Transport)?;
    Ok(Session {
        service,
        _child: guard,
    })
}
async fn http(url: &str, secrets: &Credentials) -> Result<Session, ClientError> {
    let client = reqwest::Client::builder()
        .tls_backend_preconfigured(tls()?)
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| ClientError::Transport)?;
    let mut headers = HashMap::new();
    for (name, value) in &secrets.headers {
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| ClientError::Invalid)?;
        let mut value =
            reqwest::header::HeaderValue::from_str(value).map_err(|_| ClientError::Invalid)?;
        value.set_sensitive(true);
        headers.insert(name, value);
    }
    let mut config = StreamableHttpClientTransportConfig::with_uri(url.to_owned())
        .custom_headers(headers)
        .max_concurrent_requests(1);
    config.reinit_on_expired_session = false;
    config.max_sse_event_size = 65536;
    let service =
        ().serve(StreamableHttpClientTransport::with_client(client, config))
            .await
            .map_err(|_| ClientError::Transport)?;
    Ok(Session {
        service,
        _child: ChildGuard(None),
    })
}

fn tls() -> Result<rustls::ClientConfig, ClientError> {
    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    let verifier = rustls_platform_verifier::Verifier::new(provider.clone())
        .map_err(|_| ClientError::Transport)?;
    Ok(rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| ClientError::Transport)?
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(verifier))
        .with_no_client_auth())
}

fn stdio_command(
    executable: &str,
    args: &[String],
    secrets: &Credentials,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(executable);
    command
        .args(args)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    command.envs(&secrets.env);
    command
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_environment_overrides_defaults_without_inheriting_credentials() {
        let mut secrets = Credentials::default();
        secrets.env.insert("PATH".into(), "/fixture/bin".into());
        secrets
            .env
            .insert("FIXTURE_KEY".into(), "synthetic-only".into());
        let command = stdio_command("fixture", &[], &secrets);
        let entries: std::collections::BTreeMap<_, _> = command.as_std().get_envs().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[std::ffi::OsStr::new("PATH")],
            Some(std::ffi::OsStr::new("/fixture/bin"))
        );
    }
}
