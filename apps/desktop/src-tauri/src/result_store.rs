// SPDX-License-Identifier: Apache-2.0
//! The shared result store: its memory budget comes from `settings.json`
//! (`results.memoryBudgetMb`, default 512) and it spills beyond it into a
//! per-run directory under the app cache dir (docs/06 §3). Earlier runs'
//! directories are swept at start; this run's is deleted at exit.
use std::sync::Arc;

use openrows_results::{DEFAULT_MAX_BYTES, Limits, ResultStore, SpillDir};
use tauri::Manager;

/// Smallest budget accepted from `settings.json`, in MiB.
const MIN_BUDGET_MB: u64 = 64;
/// Largest budget accepted from `settings.json`, in MiB (64 GiB).
const MAX_BUDGET_MB: u64 = 65_536;

/// The memory budget in bytes: a valid `results.memoryBudgetMb` in the
/// settings text, else the default. Invalid values are ignored, never fatal.
pub(crate) fn budget_bytes(settings: Option<&str>) -> usize {
    settings
        .and_then(budget_mb)
        .and_then(|mb| usize::try_from(mb << 20).ok())
        .unwrap_or(DEFAULT_MAX_BYTES)
}

fn budget_mb(text: &str) -> Option<u64> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let mb = value.get("results")?.get("memoryBudgetMb")?.as_u64()?;
    (MIN_BUDGET_MB..=MAX_BUDGET_MB).contains(&mb).then_some(mb)
}

/// Manage the `Arc<ResultStore>`. Runs after `settings::install`.
///
/// # Errors
/// The spike rows cannot be generated (spike mode only).
pub(crate) fn install(app: &mut tauri::App) -> tauri::Result<()> {
    let limits = Limits {
        max_bytes: budget_bytes(crate::settings::text(app).as_deref()),
        ..Limits::default()
    };
    let cache = app.path().app_cache_dir();
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    let cache = crate::automation::root(app.handle()).map_or(cache, |root| Ok(root.join("cache")));
    let spilling = cache
        .map_err(std::io::Error::other)
        .and_then(|cache| SpillDir::create(&cache.join("results")))
        .and_then(|dir| ResultStore::with_spill(limits, dir));
    let store = spilling.unwrap_or_else(|error| {
        tracing::warn!(error = %error, "result spill is unavailable; results stop at the memory budget");
        ResultStore::with_limits(limits)
    });
    tracing::info!(budget_mb = limits.max_bytes >> 20, "result store ready");
    crate::spike::seed(&store)?;
    app.manage(Arc::new(store));
    Ok(())
}

/// Drop every result and delete this run's spill directory.
pub(crate) fn exit(app: &tauri::AppHandle) {
    if let Some(store) = app.try_state::<Arc<ResultStore>>() {
        store.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_comes_from_settings_within_bounds() {
        let settings =
            |mb: &str| format!(r#"{{"openrows":1,"results":{{"memoryBudgetMb":{mb}}}}}"#);
        assert_eq!(budget_bytes(Some(&settings("1024"))), 1 << 30);
        assert_eq!(budget_bytes(Some(&settings("64"))), 64 << 20);
        for invalid in ["0", "63", "65537", "-5", "1.5", "\"512\"", "null"] {
            assert_eq!(
                budget_bytes(Some(&settings(invalid))),
                DEFAULT_MAX_BYTES,
                "{invalid}"
            );
        }
        assert_eq!(budget_bytes(Some(r#"{"openrows":1}"#)), DEFAULT_MAX_BYTES);
        assert_eq!(budget_bytes(Some("not json")), DEFAULT_MAX_BYTES);
        assert_eq!(budget_bytes(None), 512 << 20);
    }
}
