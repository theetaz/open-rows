// SPDX-License-Identifier: Apache-2.0
//! One condition-driven JavaScript request at a time over a private Unix socket.
use super::Automation;
use serde::Deserialize;
use std::time::Duration;
use tauri::Manager;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;

#[derive(Debug, thiserror::Error)]
enum TransportError {
    #[error("Automation I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Automation protocol: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Automation webview: {0}")]
    Webview(#[from] tauri::Error),
    #[error("Missing main window")]
    MissingWindow,
    #[error("Automation state unavailable")]
    State,
    #[error("Automation reply is not an object")]
    InvalidReply,
    #[error("Automation reply channel closed")]
    Closed(#[from] oneshot::error::RecvError),
}

#[derive(Deserialize)]
struct Request {
    source: String,
    #[serde(default)]
    exit: bool,
}

pub(super) async fn serve(app: tauri::AppHandle, listener: std::os::unix::net::UnixListener) {
    let Ok(listener) = UnixListener::from_std(listener) else {
        app.exit(1);
        return;
    };
    while let Ok((mut stream, _)) = listener.accept().await {
        let completed = matches!(
            tokio::time::timeout(Duration::from_secs(150), execute(&app, &mut stream)).await,
            Ok(Ok(()))
        );
        if !completed {
            app.exit(1);
            break;
        }
    }
}

async fn execute(app: &tauri::AppHandle, stream: &mut UnixStream) -> Result<(), TransportError> {
    let mut line = String::new();
    BufReader::new((&mut *stream).take(256 * 1024))
        .read_line(&mut line)
        .await?;
    let request: Request = serde_json::from_str(&line)?;
    let window = app
        .get_webview_window("main")
        .ok_or(TransportError::MissingWindow)?;
    let (sender, receiver) = oneshot::channel();
    *app.state::<Automation>()
        .pending
        .lock()
        .map_err(|_| TransportError::State)? = Some(sender);
    window.eval(format!(
        "(async()=>{{let value;try{{value={{ok:true,value:await ({})}}}}catch(error){{value={{ok:false,error:JSON.stringify(error,Object.getOwnPropertyNames(error))}}}}await window.__TAURI_INTERNALS__.invoke('automation_reply',{{value}})}})()",
        request.source
    ))?;
    let mut response = receiver.await?;
    response
        .as_object_mut()
        .ok_or(TransportError::InvalidReply)?
        .insert("pid".to_owned(), serde_json::json!(std::process::id()));
    let bytes = serde_json::to_vec(&response)?;
    stream.write_all(&bytes).await?;
    stream.shutdown().await?;
    if request.exit {
        app.exit(0);
    }
    Ok(())
}
