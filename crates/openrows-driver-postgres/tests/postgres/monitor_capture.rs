// SPDX-License-Identifier: Apache-2.0
//! Opt-in redacted Arrow captures for real-data UI verification, never a normal-test side effect.
use super::support::TestResult;
use openrows_monitor::{LiveStore, Source};
use std::{path::PathBuf, time::SystemTime};

/// Wall-clock milliseconds for actual collected fixture observations.
pub(super) fn wall_ms() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64() * 1_000.0)
}
/// Capture already-sanitised bounded Arrow only when a fixture directory was explicitly supplied.
pub(super) fn capture(store: &LiveStore, source: Source) -> TestResult {
    let Some(directory) = std::env::var_os("OPENROWS_MONITOR_CAPTURE_DIR") else {
        return Ok(());
    };
    let directory = PathBuf::from(directory);
    std::fs::create_dir_all(&directory)?;
    std::fs::write(
        directory.join(format!("{source:?}.arrow")),
        store.window(Some(source), 1_000, false)?,
    )?;
    std::fs::write(
        directory.join(format!("{source:?}-series.arrow")),
        store.window(None, 1_000, false)?,
    )?;
    Ok(())
}
