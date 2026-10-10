// SPDX-License-Identifier: Apache-2.0
//! Lazy installed-host state; M2-04 registers `DuckDB` against this shared runtime.
use openrows_host_client::DriverHostRuntime;
use tauri::Manager;

pub(crate) fn install(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = DriverHostRuntime::new(
        std::env::current_exe()?,
        app.path().app_cache_dir()?.join("hosts"),
    );
    // Developer-only explicit path, compiled out together with the override API in release.
    #[cfg(debug_assertions)]
    let runtime = match std::env::var_os("OPENROWS_DEV_HOST") {
        Some(path) => runtime.with_debug_helper(path.into())?,
        None => runtime,
    };
    app.manage(runtime);
    Ok(())
}
