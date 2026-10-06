// SPDX-License-Identifier: Apache-2.0
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Listener, Manager};

#[derive(Debug, thiserror::Error)]
enum StartupError {
    #[error("main window is missing")]
    MissingWindow,
    #[error(transparent)]
    Window(#[from] tauri::Error),
    #[error(transparent)]
    Report(#[from] std::io::Error),
    #[error(transparent)]
    Clock(#[from] std::time::SystemTimeError),
}

pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or(StartupError::MissingWindow)?;
    let handle = app.handle().clone();
    window.clone().once("openrows:first-paint-ready", move |_| {
        if reveal(&window).is_err() {
            handle.exit(1);
        }
    });
    Ok(())
}

fn reveal(window: &tauri::WebviewWindow) -> Result<(), StartupError> {
    window.show()?;
    if let Some(path) = std::env::var_os("OPENROWS_STARTUP_REPORT") {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        std::fs::write(path, timestamp.to_string())?;
    }
    Ok(())
}
