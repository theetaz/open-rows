// SPDX-License-Identifier: Apache-2.0
//! Build a workspace window exactly like the main window of `tauri.conf.json`:
//! same chrome, material and first-paint reveal, under another label.
use tauri::{Manager, WebviewWindow, WebviewWindowBuilder};

/// Where a new window appears.
pub(super) enum Placement<'a> {
    /// Its slot's saved frame, else sized to the monitor (relaunch, Dock reopen).
    Restore,
    /// The frame saved under another slot, whose workspace it adopted.
    Like(&'a str),
    /// Offset from this window, like a new document window.
    Cascade(&'a WebviewWindow),
}

/// Open the hidden window `label`, placed before its first reveal.
pub(super) fn open(
    app: &tauri::AppHandle,
    label: &str,
    placement: &Placement<'_>,
) -> tauri::Result<WebviewWindow> {
    let mut config = app
        .config()
        .app
        .windows
        .first()
        .cloned()
        .ok_or(tauri::Error::WindowNotFound)?;
    label.clone_into(&mut config.label);
    config.visible = false;
    let window = WebviewWindowBuilder::from_config(app, &config)?.build()?;
    match placement {
        Placement::Restore => crate::window_frame::place(&window)?,
        Placement::Like(other) => crate::window_frame::place_as(&window, other)?,
        Placement::Cascade(from) => crate::window_frame::cascade(&window, from)?,
    }
    crate::startup::prepare(&window, false)?;
    super::track(&window);
    if let Some(windows) = app.try_state::<super::Windows>() {
        windows.lock().focused = Some(label.to_owned());
    }
    Ok(window)
}
