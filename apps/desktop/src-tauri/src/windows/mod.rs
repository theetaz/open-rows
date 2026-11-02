// SPDX-License-Identifier: Apache-2.0
//! Workspace windows (FR-APP-03, docs/16 and docs/20 "Implemented (M5-11)").
//! Every window is an independent webview with its own workspace: tabs and
//! session are stored under its label (`workspace.rs`), its frame by the
//! window-state plugin. The labels of open windows are saved, so a relaunch
//! restores every window. Closing a window forgets its workspace unless it is
//! the last one: macOS keeps the app in the Dock and the next window (Dock
//! click, New Window, a link) reopens that workspace. Menus, links and files
//! go to the focused window.
pub(crate) mod commands;
mod create;
mod slots;
pub(crate) use slots::{is_workspace, labels};

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

use openrows_store::LocalStore;
use tauri::{Manager, WebviewWindow, WindowEvent};

use create::Placement;
use slots::MAIN;

/// Window bookkeeping shared by every window.
#[derive(Default)]
pub(crate) struct Windows(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    /// The window that last had focus.
    focused: Option<String>,
    /// Windows the user asked to close (not closed by quitting).
    closing: HashSet<String>,
    /// The app is quitting: windows going away stay saved.
    quitting: bool,
    /// Moved tabs waiting for their window, as serialised descriptors.
    inbox: HashMap<String, Vec<String>>,
}

impl Windows {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Track the main window and recreate every window open at the last quit.
pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(Windows::default());
    let main = app.get_webview_window(MAIN).ok_or("Missing main window")?;
    track(&main);
    let store = app.state::<LocalStore>();
    let mut saved = slots::load(&store);
    if let Some(first) = saved.first().filter(|label| *label != MAIN).cloned() {
        slots::adopt(&store, &first)?;
        crate::window_frame::place_as(&main, &first)?;
        saved.remove(0);
    }
    for label in saved.iter().filter(|label| *label != MAIN) {
        create::open(app.handle(), label, &Placement::Restore)?;
    }
    persist(app.handle());
    Ok(())
}

/// Workspace windows that are open, in slot order.
fn open(app: &tauri::AppHandle) -> Vec<String> {
    let windows = app.webview_windows();
    labels()
        .filter(|label| windows.contains_key(label))
        .collect()
}

/// The window menus, files and links go to: the focused one, else the first.
pub(crate) fn target(app: &tauri::AppHandle) -> Option<String> {
    let open = open(app);
    let focused = app.try_state::<Windows>()?.lock().focused.clone();
    focused
        .filter(|label| open.contains(label))
        .or_else(|| open.into_iter().next())
}

/// Whether `window` is the one that receives menus, files and links.
pub(crate) fn is_target(window: &WebviewWindow) -> bool {
    target(window.app_handle()).as_deref() == Some(window.label())
}

/// The target window, opening one first when every window is closed.
pub(crate) fn ensure_window(app: &tauri::AppHandle) -> String {
    if let Some(label) = target(app) {
        return label;
    }
    let store = app.state::<LocalStore>();
    let saved = slots::load(&store);
    let placement = match saved.first() {
        Some(last) if last != MAIN && slots::adopt(&store, last).is_ok() => Placement::Like(last),
        _ => Placement::Restore,
    };
    if let Err(error) = create::open(app, MAIN, &placement) {
        tracing::error!(%error, "Could not open a workspace window");
    }
    persist(app);
    MAIN.to_owned()
}

/// Follow focus and closing of a workspace window.
fn track(window: &WebviewWindow) {
    let app = window.app_handle().clone();
    let label = window.label().to_owned();
    window.on_window_event(move |event| {
        let windows = app.state::<Windows>();
        match event {
            WindowEvent::Focused(true) => windows.lock().focused = Some(label.clone()),
            WindowEvent::CloseRequested { .. } => {
                windows.lock().closing.insert(label.clone());
            }
            WindowEvent::Destroyed => destroyed(&app, &label),
            _ => {}
        }
    });
}

fn destroyed(app: &tauri::AppHandle, label: &str) {
    let windows = app.state::<Windows>();
    let (closing, quitting) = {
        let mut inner = windows.lock();
        inner.inbox.remove(label);
        (inner.closing.remove(label), inner.quitting)
    };
    let others: Vec<_> = open(app)
        .into_iter()
        .filter(|other| other != label)
        .collect();
    if quitting || others.is_empty() {
        // Quitting keeps every window; the last window's workspace waits for the next one.
        return;
    }
    if closing && let Err(error) = slots::forget(&app.state::<LocalStore>(), label) {
        tracing::warn!(%error, "Could not forget a closed window");
    }
    save(app, &others);
}

/// Save the open windows for the next launch.
fn persist(app: &tauri::AppHandle) {
    save(app, &open(app));
}

fn save(app: &tauri::AppHandle, open: &[String]) {
    if let Err(error) = slots::save(&app.state::<LocalStore>(), open) {
        tracing::warn!(%error, "Could not save the open windows");
    }
}

/// macOS conventions: closing the last window keeps the app running; a Dock
/// click with no window opens one. Quitting keeps every window saved.
pub(crate) fn on_run_event(app: &tauri::AppHandle, event: &tauri::RunEvent) {
    match event {
        #[cfg(target_os = "macos")]
        tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } => api.prevent_exit(),
        tauri::RunEvent::ExitRequested { code: Some(_), .. } | tauri::RunEvent::Exit => {
            if let Some(windows) = app.try_state::<Windows>() {
                windows.lock().quitting = true;
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } => {
            ensure_window(app);
        }
        _ => {}
    }
}
