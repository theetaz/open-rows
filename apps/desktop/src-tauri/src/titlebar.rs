// SPDX-License-Identifier: Apache-2.0
//! Toolbar inset after the native traffic lights, measured from the real buttons.
//!
//! Traffic-light size and spacing differ between macOS releases (macOS 26 draws
//! larger buttons), so a fixed inset leaves the first toolbar group too close
//! on some systems. The zoom button's right edge plus the toolbar's group gap
//! keeps the same spacing after the lights as between toolbar groups.

/// Gap between toolbar groups (`--or-space-8`), repeated after the lights.
const GROUP_GAP: f64 = 8.0;
/// Toolbar edge padding (`--or-space-10`) when full screen hides the lights.
const EDGE_PADDING: f64 = 10.0;

/// Script that sets `--or-traffic-light-clearance`: after a zoom button ending
/// at `zoom_right`, or the plain edge padding when the lights are hidden.
fn clearance_script(zoom_right: f64, fullscreen: bool) -> String {
    let clearance = if fullscreen {
        EDGE_PADDING
    } else {
        (zoom_right + GROUP_GAP).ceil()
    };
    format!(
        "document.documentElement.style.setProperty('--or-traffic-light-clearance', '{clearance}px')"
    )
}

/// Right edge of the zoom button in window points, when a window has one.
#[cfg(target_os = "macos")]
fn zoom_right() -> Option<f64> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSWindowButton};
    let application = NSApplication::sharedApplication(MainThreadMarker::new()?);
    application.windows().iter().find_map(|window| {
        let zoom = window.standardWindowButton(NSWindowButton::ZoomButton)?;
        let frame = zoom.frame();
        Some(frame.origin.x + frame.size.width)
    })
}

#[cfg(not(target_os = "macos"))]
fn zoom_right() -> Option<f64> {
    None
}

/// Apply the measured inset; the CSS token stays as the fallback when no button is found.
pub(crate) fn synchronize(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    match zoom_right() {
        Some(right) => window.eval(clearance_script(right, window.is_fullscreen()?)),
        None => Ok(()),
    }
}

/// Re-measure when the window enters or leaves full screen (both resize it).
pub(crate) fn follow(window: &tauri::WebviewWindow) {
    let handle = window.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Resized(_)) {
            // A failed update keeps the previous inset; the next resize retries.
            synchronize(&handle).unwrap_or_default();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::clearance_script;

    #[test]
    fn clearance_repeats_the_group_gap_after_the_zoom_button() {
        assert!(clearance_script(77.5, false).ends_with("'86px')"));
        assert!(clearance_script(68.0, false).ends_with("'76px')"));
    }

    #[test]
    fn full_screen_uses_the_plain_edge_padding() {
        assert!(clearance_script(77.5, true).ends_with("'10px')"));
    }
}
