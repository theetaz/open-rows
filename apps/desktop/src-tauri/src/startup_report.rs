// SPDX-License-Identifier: Apache-2.0
//! Optional benchmark instrumentation; the disabled path never reads the clock.
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

static REPORT: OnceLock<Option<Mutex<Report>>> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    MainEntry,
    BuilderStart,
    WebviewCreated,
    SetupComplete,
    PageLoadFinished,
    FirstPaintReady,
    WindowShown,
}

const NAMES: [&str; 7] = [
    "main_entry",
    "builder_start",
    "webview_created",
    "setup_complete",
    "page_load_finished",
    "first_paint_ready",
    "window_shown",
];

#[derive(serde::Serialize)]
struct Snapshot {
    schema_version: u8,
    clock: &'static str,
    pid: u32,
    timestamps_ns: BTreeMap<&'static str, u128>,
}

struct Report {
    path: Option<PathBuf>,
    timestamps: [Option<u128>; 7],
}

impl Report {
    fn new(path: PathBuf) -> Self {
        let mut report = Self {
            path: Some(path),
            timestamps: [None; 7],
        };
        report.timestamps[Phase::MainEntry as usize] = Some(timestamp());
        report
    }

    fn snapshot(&self) -> Option<Snapshot> {
        let values: Option<Vec<_>> = self.timestamps.iter().copied().collect();
        Some(Snapshot {
            schema_version: 1,
            clock: "CLOCK_MONOTONIC",
            pid: std::process::id(),
            timestamps_ns: NAMES.into_iter().zip(values?).collect(),
        })
    }

    fn record(&mut self, phase: Phase) -> Result<(), ReportError> {
        self.timestamps[phase as usize].get_or_insert_with(timestamp);
        if let Some(snapshot) = self.snapshot()
            && let Some(path) = self.path.take()
        {
            let partial = path.with_extension("partial");
            std::fs::write(&partial, serde_json::to_vec(&snapshot)?)?;
            std::fs::rename(partial, path)?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
enum ReportError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(unix)]
fn timestamp() -> u128 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    // Monotonic seconds and nanoseconds are nonnegative by the OS contract.
    u128::from(now.tv_sec.unsigned_abs()) * 1_000_000_000 + u128::from(now.tv_nsec.unsigned_abs())
}

#[cfg(not(unix))]
fn timestamp() -> u128 {
    0
}

pub(crate) fn initialize() {
    REPORT.get_or_init(|| {
        // These benchmarks are Unix-only; unsupported platforms keep normal behaviour.
        if cfg!(unix) {
            configured_report(std::env::var_os("OPENROWS_STARTUP_REPORT"))
        } else {
            None
        }
    });
}

fn configured_report(path: Option<OsString>) -> Option<Mutex<Report>> {
    path.map(|path| Mutex::new(Report::new(path.into())))
}

pub(crate) fn enabled() -> bool {
    REPORT.get().is_some_and(Option::is_some)
}

pub(crate) fn record(phase: Phase) {
    if let Some(Some(report)) = REPORT.get() {
        // A broken benchmark report must fail the run, never silently omit a phase.
        let result = report.lock().map(|mut report| report.record(phase));
        if !matches!(result, Ok(Ok(()))) {
            std::process::exit(1);
        }
    }
}

pub(crate) fn instrument(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    if !enabled() {
        return builder;
    }
    builder.plugin(
        tauri::plugin::Builder::<tauri::Wry>::new("startup-report")
            .on_webview_ready(|webview| {
                if webview.label() == "main" {
                    record(Phase::WebviewCreated);
                }
            })
            .on_page_load(|webview, payload| {
                if webview.label() == "main"
                    && payload.event() == tauri::webview::PageLoadEvent::Finished
                {
                    record(Phase::PageLoadFinished);
                }
            })
            .build(),
    )
}

pub(crate) fn watch_stop(handle: tauri::AppHandle) {
    let Some(path) = std::env::var_os("OPENROWS_STARTUP_REPORT") else {
        return;
    };
    let path = PathBuf::from(path).with_extension("stop");
    std::thread::spawn(move || {
        let (_sender, receiver) = std::sync::mpsc::channel::<()>();
        while !path.exists() {
            let _timeout = receiver.recv_timeout(std::time::Duration::from_millis(50));
        }
        handle.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_contains_every_monotonic_phase() -> Result<(), Box<dyn std::error::Error>> {
        let report = Report {
            path: None,
            timestamps: [Some(123); 7],
        };
        let value = serde_json::to_value(report.snapshot())?;
        assert_eq!(value["clock"], "CLOCK_MONOTONIC");
        assert_eq!(value["schema_version"], 1);
        for name in NAMES {
            assert_eq!(value["timestamps_ns"][name], 123);
        }
        assert_eq!(
            value["timestamps_ns"].as_object().map(serde_json::Map::len),
            Some(7)
        );
        Ok(())
    }

    #[test]
    fn incomplete_report_is_not_serialized() {
        let report = Report {
            path: None,
            timestamps: [None; 7],
        };
        assert!(report.snapshot().is_none());
    }

    #[test]
    fn absent_report_path_disables_recording() {
        assert!(configured_report(None).is_none());
    }

    #[test]
    fn report_is_written_once_and_atomically() -> Result<(), Box<dyn std::error::Error>> {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!(".report-test-{}", std::process::id()));
        std::fs::create_dir(&directory)?;
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let path = directory.join("startup.json");
            let mut report = Report {
                path: Some(path.clone()),
                timestamps: [Some(123); 7],
            };
            report.timestamps[Phase::WindowShown as usize] = None;
            assert!(!path.exists());
            report.record(Phase::WindowShown)?;
            let first = std::fs::read(&path)?;
            report.timestamps[Phase::MainEntry as usize] = Some(456);
            report.record(Phase::WindowShown)?;
            assert_eq!(std::fs::read(&path)?, first);
            assert!(!path.with_extension("partial").exists());
            Ok(())
        })();
        std::fs::remove_dir_all(directory)?;
        result
    }
}
