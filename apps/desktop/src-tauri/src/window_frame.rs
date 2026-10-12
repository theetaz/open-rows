// SPDX-License-Identifier: Apache-2.0
//! Main-window frame: fit the monitor on first launch, restore the saved frame afterwards.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{LogicalSize, Manager, PhysicalSize, WebviewWindow, WindowEvent};
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};
use tokio::sync::Notify;

/// Persisted properties; visibility stays with the first-paint reveal in `startup`.
const STATE_FLAGS: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED);
const MAIN_WINDOW: &str = "main";
/// Share of the monitor work area used on first launch.
const FILL: f64 = 0.85;
const MAX_SIZE: (f64, f64) = (1680.0, 1050.0);
/// Mirrors `minWidth`/`minHeight` in `tauri.conf.json`.
const MIN_SIZE: (f64, f64) = (900.0, 600.0);
/// Quiet period after the last move or resize before the frame is saved.
const SAVE_DELAY: Duration = Duration::from_millis(500);

/// Saved outer position and inner size in physical pixels, as written by the plugin.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
struct Frame {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// Window-state plugin that tracks the main window and saves it on exit.
/// The initial restore is skipped so [`place`] can choose between restoring and fitting.
pub(crate) fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let builder = tauri_plugin_window_state::Builder::new();
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    let builder = match std::env::var("OPENROWS_E2E_ROOT") {
        Ok(root) => builder.with_filename(format!("{root}/window-state.json")),
        Err(_) => builder,
    };
    builder
        .with_state_flags(STATE_FLAGS)
        .skip_initial_state(MAIN_WINDOW)
        .build()
}

/// Size and position the hidden main window before its first reveal.
pub(crate) fn place(window: &WebviewWindow) -> tauri::Result<()> {
    match saved_frame(window) {
        Some(frame) if is_on_screen(frame, &work_areas(window)?) => {
            window.restore_state(STATE_FLAGS)
        }
        _ => fit_to_monitor(window),
    }
}

/// Save the frame once the user stops moving or resizing the window, so it also
/// survives a killed process (`Ctrl+C` in `just dev`), not only a normal quit.
/// One long-lived task debounces all events; no task is spawned per event.
pub(crate) fn save_when_settled(window: &WebviewWindow) {
    let changes = Arc::new(Notify::new());
    let signal = Arc::clone(&changes);
    window.on_window_event(move |event| {
        if changes_frame(event) {
            signal.notify_one();
        }
    });
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            settle(&changes, SAVE_DELAY).await;
            let target = window.clone();
            // The plugin holds its cache lock while reading window state; saving on the
            // main thread keeps it from contending with its own move/resize handlers.
            if window
                .run_on_main_thread(move || save_if_visible(&target))
                .is_err()
            {
                break;
            }
        }
    });
}

fn changes_frame(event: &WindowEvent) -> bool {
    matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_))
}

/// Wait for a change, then until no further change arrives for `quiet`.
async fn settle(changes: &Notify, quiet: Duration) {
    changes.notified().await;
    while tokio::time::timeout(quiet, changes.notified())
        .await
        .is_ok()
    {}
}

/// The hidden pre-paint window is skipped: its frame is not the user's choice yet.
fn save_if_visible(window: &WebviewWindow) {
    if window.is_visible().unwrap_or(false) {
        // A failed save keeps the previous file; the next settle or the quit save retries.
        let _ = window.app_handle().save_window_state(STATE_FLAGS);
    }
}

/// A missing or unreadable state file means there is nothing to restore.
fn saved_frame(window: &WebviewWindow) -> Option<Frame> {
    let app = window.app_handle();
    let path = app.path().app_config_dir().ok()?.join(app.filename());
    let bytes = std::fs::read(path).ok()?;
    let mut states: HashMap<String, Frame> = serde_json::from_slice(&bytes).ok()?;
    states
        .remove(MAIN_WINDOW)
        .filter(|frame| frame.width > 0 && frame.height > 0)
}

fn work_areas(window: &WebviewWindow) -> tauri::Result<Vec<Frame>> {
    Ok(window
        .available_monitors()?
        .iter()
        .map(|monitor| {
            let area = monitor.work_area();
            Frame {
                x: area.position.x,
                y: area.position.y,
                width: area.size.width,
                height: area.size.height,
            }
        })
        .collect())
}

fn fit_to_monitor(window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = match window.current_monitor()? {
        Some(monitor) => Some(monitor),
        None => window.primary_monitor()?,
    };
    if let Some(monitor) = monitor {
        let size = initial_window_size(monitor.work_area().size, monitor.scale_factor());
        window.set_size(size)?;
    }
    window.center()
}

/// First-launch logical size: 85% of the work area, capped for large screens and
/// never below the configured minimum.
fn initial_window_size(work_area: PhysicalSize<u32>, scale: f64) -> LogicalSize<f64> {
    let available: LogicalSize<f64> = work_area.to_logical(scale);
    let fit = |extent: f64, (min, max): (f64, f64)| (extent * FILL).round().clamp(min, max);
    LogicalSize::new(
        fit(available.width, (MIN_SIZE.0, MAX_SIZE.0)),
        fit(available.height, (MIN_SIZE.1, MAX_SIZE.1)),
    )
}

/// A saved frame is usable when at least a quarter of its width and height overlap
/// one monitor's work area; otherwise its monitor has likely been unplugged.
fn is_on_screen(frame: Frame, screens: &[Frame]) -> bool {
    let overlap = |start: i32, len: u32, other: i32, other_len: u32| {
        let end = (i64::from(start) + i64::from(len)).min(i64::from(other) + i64::from(other_len));
        (end - i64::from(start.max(other))).max(0)
    };
    screens.iter().any(|screen| {
        let width = overlap(frame.x, frame.width, screen.x, screen.width);
        let height = overlap(frame.y, frame.height, screen.y, screen.height);
        width * 4 >= i64::from(frame.width) && height * 4 >= i64::from(frame.height)
    })
}

#[cfg(test)]
mod tests {
    use super::{Frame, changes_frame, initial_window_size, is_on_screen, settle};
    use std::sync::Arc;
    use std::time::Duration;
    use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, WindowEvent};
    use tokio::sync::Notify;

    fn frame(x: i32, y: i32, width: u32, height: u32) -> Frame {
        Frame {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn laptop_work_area_gets_a_fitting_window() {
        let size = initial_window_size(PhysicalSize::new(2880, 1750), 2.0);
        assert_eq!(size, LogicalSize::new(1224.0, 744.0));
    }

    #[test]
    fn large_monitor_is_capped() {
        let size = initial_window_size(PhysicalSize::new(5120, 2840), 2.0);
        assert_eq!(size, LogicalSize::new(1680.0, 1050.0));
    }

    #[test]
    fn small_monitor_never_goes_below_the_minimum() {
        let size = initial_window_size(PhysicalSize::new(1024, 640), 1.0);
        assert_eq!(size, LogicalSize::new(900.0, 600.0));
    }

    #[test]
    fn frame_on_a_connected_monitor_is_restored() {
        let screens = [frame(0, 0, 2560, 1440), frame(2560, 0, 1920, 1080)];
        assert!(is_on_screen(frame(2700, 100, 1600, 900), &screens));
        assert!(is_on_screen(frame(-300, 50, 1200, 800), &screens));
    }

    #[test]
    fn frame_on_an_unplugged_monitor_is_not_restored() {
        let screens = [frame(0, 0, 2560, 1440)];
        assert!(!is_on_screen(frame(2600, 100, 1600, 900), &screens));
        assert!(!is_on_screen(frame(2400, 1300, 1600, 900), &screens));
        assert!(!is_on_screen(frame(0, 0, 1600, 900), &[]));
    }

    #[test]
    fn only_moves_and_resizes_trigger_a_save() {
        assert!(changes_frame(&WindowEvent::Moved(PhysicalPosition::new(
            4, 8
        ))));
        assert!(changes_frame(&WindowEvent::Resized(PhysicalSize::new(
            900, 600
        ))));
        assert!(!changes_frame(&WindowEvent::Focused(true)));
    }

    async fn notify_three_times(signal: Arc<Notify>) {
        for _ in 0..3 {
            signal.notify_one();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_burst_of_changes_settles_once_after_the_last_change() {
        let changes = Arc::new(Notify::new());
        let signal = Arc::clone(&changes);
        // Virtual time tests debounce semantics independently of scheduler/clock jitter.
        let started = tokio::time::Instant::now();
        let burst = tokio::spawn(notify_three_times(signal));
        settle(&changes, Duration::from_millis(50)).await;
        assert!(started.elapsed() >= Duration::from_millis(90));
        assert!(burst.await.is_ok());
        let leftover = tokio::time::timeout(Duration::from_millis(30), changes.notified()).await;
        assert!(leftover.is_err(), "burst produced a second save");
    }
}
