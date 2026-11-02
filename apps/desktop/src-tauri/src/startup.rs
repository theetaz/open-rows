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
    prepare(&window, true)?;
    startup_report::record(Phase::SetupComplete);
    Ok(())
}

/// Shared setup of every workspace window: frame saving, traffic-light inset
/// and material, then reveal once its webview painted. Only the launch window
/// (`launch`) records startup phases and prewarms the driver host.
pub(crate) fn prepare(window: &tauri::WebviewWindow, launch: bool) -> tauri::Result<()> {
    crate::window_frame::save_when_settled(window);
    crate::titlebar::follow(window);
    #[cfg(target_os = "macos")]
    crate::material::install(window)?;
    let handle = window.app_handle().clone();
    let shown = window.clone();
    window.once("openrows:first-paint-ready", move |_| {
        if launch {
            startup_report::record(Phase::FirstPaintReady);
        }
        if reveal(&shown).is_err() {
            handle.exit(1);
            return;
        }
        if !launch {
            return;
        }
        startup_report::record(Phase::WindowShown);
        crate::driver_host::after_paint(&shown);
        if startup_report::enabled() {
            startup_report::watch_stop(handle);
        }
    });
    Ok(())
}

fn reveal(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    #[cfg(target_os = "macos")]
    crate::material::synchronize(window)?;
    crate::titlebar::synchronize(window)?;
    window.show()
}
