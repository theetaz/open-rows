// SPDX-License-Identifier: Apache-2.0
//! Installed-host state and database-free prewarm after visible paint and idle.
use openrows_host_client::DriverHostRuntime;
use tauri::{Listener, Manager};
mod bench;

pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let cache = app.path().app_cache_dir()?;
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    let cache = crate::automation::root(app.handle()).map_or(cache, |root| root.join("cache"));
    let runtime = DriverHostRuntime::new(std::env::current_exe()?, cache.join("hosts"));
    // Developer-only explicit path, compiled out together with the override API in release.
    #[cfg(debug_assertions)]
    let runtime = match std::env::var_os("OPENROWS_DEV_HOST") {
        Some(path) => runtime.with_debug_helper(path.into())?,
        None => runtime,
    };
    app.manage(runtime);
    Ok(())
}

pub(crate) fn after_paint(window: &tauri::WebviewWindow) {
    let handle = window.app_handle().clone();
    window.once("openrows:driver-host-idle", move |_| {
        tauri::async_runtime::spawn(async move {
            let runtime = handle.state::<DriverHostRuntime>();
            if let Some(report) = runtime.prewarm().await {
                record_prewarm(report);
                bench::measure(&runtime, report).await;
            }
        });
    });
}

fn record_prewarm(report: openrows_host_client::PrewarmReport) {
    use openrows_host_client::PrewarmOutcome;
    let duration_ms = report.elapsed.as_secs_f64() * 1000.0;
    let handshake_ms = report.handshake.map(|time| time.as_secs_f64() * 1000.0);
    match report.outcome {
        PrewarmOutcome::Ready => tracing::info!(
            duration_ms,
            handshake_ms,
            outcome = "ready",
            "driver host prewarm"
        ),
        PrewarmOutcome::Skipped(reason) | PrewarmOutcome::Failed(reason) => tracing::warn!(
            duration_ms,
            handshake_ms,
            outcome = ?report.outcome,
            kind = ?reason,
            "driver host prewarm"
        ),
    }
}
