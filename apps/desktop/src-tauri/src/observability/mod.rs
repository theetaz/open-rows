// SPDX-License-Identifier: Apache-2.0
//! Observability (docs/14 §6–§7, M1-28): `tracing` written as scrubbed JSON
//! lines to a daily rotating file, a crash marker, and a panic hook.
//!
//! Call sites log typed kinds, counts, durations and identifiers. They never
//! log SQL text or values, connection URLs, passwords, keys, or driver error
//! messages (which can echo their input). [`scrub`] removes what slips
//! through. A disabled call site costs only `tracing`'s cached interest check,
//! because no subscriber is installed or its level filters the event out.
pub(crate) mod commands;
mod level;
mod marker;
mod panic;
mod scrub;
mod sink;
#[cfg(test)]
mod tests;

use std::fs;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};
use tracing::level_filters::LevelFilter;

/// What this launch knows about logging and the previous run.
pub(crate) struct Diagnostics {
    /// The previous run ended without a clean exit.
    crashed: bool,
    /// Folder of the daily log files (`~/Library/Logs/<identifier>` on macOS).
    log_dir: PathBuf,
    /// Crash marker of this run, removed on a clean exit.
    marker: PathBuf,
}

/// Install the panic hook. Called first thing in `run`, before any thread starts.
pub(crate) fn install_panic_hook() {
    panic::install();
}

/// Start the log file, claim the crash marker and manage [`Diagnostics`].
///
/// Logging is best effort: when the log folder cannot be used the app runs
/// without a log file and says so on stderr, rather than refusing to start.
///
/// # Errors
/// The platform directories cannot be resolved.
pub(crate) fn install(app: &mut tauri::App) -> tauri::Result<()> {
    let log_dir = app.path().app_log_dir()?;
    #[cfg(all(feature = "test-automation", debug_assertions, unix))]
    let log_dir = crate::automation::root(app.handle()).map_or(log_dir, |root| root.join("logs"));
    let data = app.state::<crate::locations::Locations>().data.clone();
    let marker = data.join(marker::FILE_NAME);
    // A missing or unreadable settings file only means the default level applies.
    let settings = fs::read_to_string(data.join("settings.json")).ok();
    let level = level::resolve(
        std::env::var(level::ENV_VAR).ok().as_deref(),
        settings.as_deref(),
    );
    if let Err(error) = start(&log_dir, level) {
        report_unavailable(&error);
    }
    let crashed = match marker::begin(&marker) {
        Ok(crashed) => crashed,
        Err(error) => {
            tracing::warn!(error = %error, "crash marker unavailable");
            false
        }
    };
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        log_level = %level,
        "OpenRows started"
    );
    if crashed {
        tracing::warn!("the previous run did not exit cleanly");
    }
    app.manage(Diagnostics {
        crashed,
        log_dir,
        marker,
    });
    Ok(())
}

/// Record a clean exit: the crash marker goes, so the next launch is not a recovery.
pub(crate) fn exit(app: &AppHandle) {
    let diagnostics = app.state::<Diagnostics>();
    tracing::info!("OpenRows stopped");
    if let Err(error) = marker::end(&diagnostics.marker) {
        tracing::warn!(error = %error, "could not remove the crash marker");
    }
}

/// Install the global subscriber that writes to the log folder.
fn start(dir: &Path, level: LevelFilter) -> Result<(), StartError> {
    fs::create_dir_all(dir)?;
    let subscriber = sink::rolling(dir, level)?;
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(())
}

/// The only channel left when the log file cannot be created.
#[expect(
    clippy::print_stderr,
    reason = "logging itself is unavailable, so stderr is the last resort"
)]
fn report_unavailable(error: &StartError) {
    eprintln!("OpenRows: logging is unavailable: {error}");
}

#[derive(Debug, thiserror::Error)]
enum StartError {
    #[error("the log folder cannot be created: {0}")]
    Folder(#[from] std::io::Error),
    #[error("the log file cannot be created: {0}")]
    File(#[from] tracing_appender::rolling::InitError),
    #[error("logging is already set up: {0}")]
    Global(#[from] tracing::subscriber::SetGlobalDefaultError),
}
