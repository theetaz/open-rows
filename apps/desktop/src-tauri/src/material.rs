// SPDX-License-Identifier: Apache-2.0
//! Native macOS material behind the transparent webview; no custom unsafe bridge.
use objc2_app_kit::NSWorkspace;
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
    let effects = if reduced {
        None
    } else {
        Some(WindowEffectsConfig {
            effects: vec![Effect::Sidebar],
            state: Some(EffectState::FollowsWindowActiveState),
            radius: Some(26.0),
            ..WindowEffectsConfig::default()
        })
    };
    window.set_effects(effects)?;
    window.eval(transparency_script(reduced))
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
}
