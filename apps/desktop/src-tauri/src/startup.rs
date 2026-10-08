// SPDX-License-Identifier: Apache-2.0
use crate::startup_report::{self, Phase};
use tauri::{Listener, Manager};

#[derive(Debug, thiserror::Error)]
enum StartupError {
    #[error("main window is missing")]
    MissingWindow,
}

pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let window = app
        .get_webview_window("main")
        .ok_or(StartupError::MissingWindow)?;
    crate::window_frame::place(&window)?;
    crate::window_frame::save_when_settled(&window);
    crate::titlebar::follow(&window);
    #[cfg(target_os = "macos")]
    crate::material::install(&window)?;
    let handle = app.handle().clone();
    window.clone().once("openrows:first-paint-ready", move |_| {
        startup_report::record(Phase::FirstPaintReady);
        if reveal(&window).is_err() {
            handle.exit(1);
            return;
        }
        startup_report::record(Phase::WindowShown);
        if startup_report::enabled() {
            startup_report::watch_stop(handle);
        }
    });
    startup_report::record(Phase::SetupComplete);
    Ok(())
}

fn reveal(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    crate::material::synchronize(window)?;
    crate::titlebar::synchronize(window)?;
    window.show()
}
