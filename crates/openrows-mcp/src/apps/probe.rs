// SPDX-License-Identifier: Apache-2.0
//! Drive the bridge executable itself: save or delete its keychain token and
//! run a harmless tool through it exactly as an agent app would.
use super::{AppError, BridgeTarget};
use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, ClientConfig, Implementation},
};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::io::AsyncWriteExt;

/// Hand a token to `openrows-mcp --client ID --store` over stdin.
/// # Errors
/// The bridge did not start or could not write the keychain item.
pub async fn store_token(bridge: &Path, client: &str, token: &str) -> Result<(), AppError> {
    let mut child = bridge_command(bridge, &["--client", client, "--store"])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|_| AppError::BridgeMissing)?;
    let mut stdin = child.stdin.take().ok_or(AppError::Keychain)?;
    stdin
        .write_all(format!("{token}\n").as_bytes())
        .await
        .map_err(|_| AppError::Keychain)?;
    drop(stdin);
    finish(child).await
}
/// Delete a client's keychain item through the bridge that created it.
/// # Errors
/// The bridge did not start or could not delete the item.
pub async fn forget_token(bridge: &Path, client: &str) -> Result<(), AppError> {
    let child = bridge_command(bridge, &["--client", client, "--forget"])
        .stdin(Stdio::null())
        .spawn()
        .map_err(|_| AppError::BridgeMissing)?;
    finish(child).await
}
/// Start the registered launch line and call `list_connections` through it.
/// Returns the tool's own text (fixed refusal text or scoped connection JSON).
/// # Errors
/// The bridge could not start, authenticate or reach the running app.
pub async fn test_bridge(target: &BridgeTarget) -> Result<String, AppError> {
    let mut child = bridge_command(&target.bridge, &[])
        .args(target.args())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| AppError::BridgeMissing)?;
    let stdout = child.stdout.take().ok_or(AppError::BridgeMissing)?;
    let stdin = child.stdin.take().ok_or(AppError::BridgeMissing)?;
    let work = async {
        let service = client_config()
            .serve((stdout, stdin))
            .await
            .map_err(|_| unreachable_app())?;
        let result = service
            .call_tool(CallToolRequestParams::new("list_connections"))
            .await
            .map_err(|_| unreachable_app())?;
        service.cancellation_token().cancel();
        Ok(result
            .content
            .iter()
            .filter_map(|block| block.as_text().map(|text| text.text.clone()))
            .collect::<Vec<_>>()
            .join("\n"))
    };
    let outcome = tokio::time::timeout(Duration::from_secs(30), work)
        .await
        .map_err(|_| AppError::Timeout)?;
    let _ = child.kill().await;
    outcome
}
fn unreachable_app() -> AppError {
    AppError::Bridge("check that OpenRows is unlocked and the MCP server is on".into())
}
fn client_config() -> ClientConfig {
    let mut info = Implementation::default();
    "OpenRows Apps test".clone_into(&mut info.name);
    env!("CARGO_PKG_VERSION").clone_into(&mut info.version);
    let mut config = ClientConfig::default();
    config.client_info = info;
    config
}
fn bridge_command(bridge: &Path, args: &[&str]) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(bridge);
    command
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    command
}
async fn finish(mut child: tokio::process::Child) -> Result<(), AppError> {
    let status = tokio::time::timeout(Duration::from_secs(30), child.wait())
        .await
        .map_err(|_| AppError::Timeout)?
        .map_err(|_| AppError::Keychain)?;
    status.success().then_some(()).ok_or(AppError::Keychain)
}
