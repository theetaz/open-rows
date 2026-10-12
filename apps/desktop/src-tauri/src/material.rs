// SPDX-License-Identifier: Apache-2.0
//! Native macOS material behind the transparent webview; no custom unsafe bridge.
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSProcessInfo;
use tauri::Manager;
use tauri::utils::config::WindowEffectsConfig;
use tauri::window::{Effect, EffectState};

pub(crate) fn install(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    synchronize(window)?;
    let handle = window.clone();
    let application = window.app_handle().clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Focused(true)) && synchronize(&handle).is_err() {
            application.exit(1);
        }
    });
    Ok(())
}

pub(crate) fn synchronize(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let reduced = NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceTransparency();
    let radius = corner_radius(
        NSProcessInfo::processInfo()
            .operatingSystemVersion()
            .majorVersion,
    );
    let effects = if reduced {
        None
    } else {
        Some(WindowEffectsConfig {
            effects: vec![Effect::Sidebar],
            state: Some(EffectState::FollowsWindowActiveState),
            radius: Some(radius),
            ..WindowEffectsConfig::default()
        })
    };
    window.set_effects(effects)?;
    window.eval(transparency_script(reduced))?;
    window.eval(radius_script(radius))
}

/// The native corner radius of a titled window without an `NSToolbar`: 16 pt
/// since macOS 26, 10 pt before. The webview frame and material must match it
/// or the transparent corners show through.
fn corner_radius(major: isize) -> f64 {
    if major >= 26 { 16.0 } else { 10.0 }
}

fn radius_script(radius: f64) -> String {
    format!("document.documentElement.style.setProperty('--or-radius-window', '{radius}px')")
}

fn transparency_script(reduced: bool) -> &'static str {
    if reduced {
        "document.documentElement.dataset.transparency = 'reduced'"
    } else {
        "delete document.documentElement.dataset.transparency"
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn transparency_preference_drives_solid_css_without_ipc() {
        assert!(super::transparency_script(true).contains("'reduced'"));
        assert!(super::transparency_script(false).starts_with("delete "));
    }

    #[test]
    fn window_radius_follows_the_native_titled_window_corner() {
        assert!((super::corner_radius(26) - 16.0).abs() < f64::EPSILON);
        assert!((super::corner_radius(15) - 10.0).abs() < f64::EPSILON);
        assert!(super::radius_script(16.0).ends_with("'16px')"));
    }
}
