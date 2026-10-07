// SPDX-License-Identifier: Apache-2.0
use openrows_results::{ResultStore, synthetic};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Serialize, Type, thiserror::Error)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub(crate) enum SpikeError {
    #[error("Spike mode is disabled")]
    Disabled,
    #[error("OPENROWS_SPIKE_REPORT is required")]
    MissingPath,
    #[error("{0}")]
    Failed(String),
}

/// Explicit release-app measurement modes; any other value leaves spike mode off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpikeMode {
    Ipc,
    Grid,
    Editor,
}

impl SpikeMode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "ipc" => Some(Self::Ipc),
            "grid" => Some(Self::Grid),
            "editor" => Some(Self::Editor),
            _ => None,
        }
    }

    /// Only the tile-based spikes read the synthetic result store.
    fn needs_rows(self) -> bool {
        self != Self::Editor
    }
}

fn mode() -> Option<SpikeMode> {
    std::env::var("OPENROWS_SPIKE")
        .ok()
        .and_then(|value| SpikeMode::parse(&value))
}

fn enabled() -> bool {
    mode().is_some()
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn spike_start(
    window: tauri::WebviewWindow,
) -> Result<Option<String>, SpikeError> {
    if !enabled() {
        return Ok(None);
    }
    spike_progress("page-script-started", "")?;
    let mode =
        std::env::var("OPENROWS_SPIKE").map_err(|error| SpikeError::Failed(error.to_string()))?;
    spike_progress("spike-flag-seen", &mode)?;
    window
        .show()
        .map_err(|error| SpikeError::Failed(error.to_string()))?;
    window
        .set_focus()
        .map_err(|error| SpikeError::Failed(error.to_string()))?;
    spike_progress("window-shown", "")?;
    Ok(Some(mode))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn spike_progress(stage: &str, detail: &str) -> Result<(), SpikeError> {
    append_progress(&report_path()?, stage, detail)
}

fn append_progress(path: &Path, stage: &str, detail: &str) -> Result<(), SpikeError> {
    let line = format!("{stage}: {detail}").replace(['\r', '\n'], " ");
    writeln!(std::io::stderr().lock(), "OpenRows spike {line}")
        .map_err(|error| SpikeError::Failed(error.to_string()))?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.with_extension("progress"))
        .map_err(|error| SpikeError::Failed(error.to_string()))?;
    writeln!(file, "{line}").map_err(|error| SpikeError::Failed(error.to_string()))
}

pub(crate) fn configure(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Result<tauri::Builder<tauri::Wry>> {
    let mut store = ResultStore::default();
    if mode().is_some_and(SpikeMode::needs_rows) {
        spike_progress("native-configuring", "generating synthetic rows")
            .map_err(|error| tauri::Error::Anyhow(error.into()))?;
        let source = synthetic::batch(
            synthetic::SPIKE_ROWS,
            synthetic::SPIKE_COLS,
            synthetic::SPIKE_SEED,
        )
        .map_err(|error| tauri::Error::Anyhow(error.into()))?;
        store
            .insert("spike".to_owned(), vec![source])
            .map_err(|error| tauri::Error::Anyhow(error.into()))?;
    }
    Ok(builder.manage(Arc::new(store)))
}

#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TileTiming {
    row_start: u32,
    invoke_ms: f64,
    decode_ms: f64,
    worker_decode_ms: f64,
    total_ms: f64,
}

#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageTimer {
    resolution_ms: Option<f64>,
    cross_origin_isolated: bool,
}

#[derive(Serialize, Deserialize, Type)]
pub(crate) struct SpikeReport {
    samples: Vec<TileTiming>,
    timer: PageTimer,
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grid: Option<Vec<crate::grid_report::GridRun>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    editor: Option<Vec<crate::editor_report::EditorRun>>,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn spike_finish(app: tauri::AppHandle, report: SpikeReport) -> Result<(), SpikeError> {
    let path = report_path()?;
    let status = i32::from(report.error.is_some());
    let stage_summary = summaries(&report.samples);
    let method = method(&report);
    let samples =
        serde_json::to_value(report).map_err(|error| SpikeError::Failed(error.to_string()))?;
    let payload = serde_json::json!({
        "schema_version": 1, "pid": std::process::id(), "seed": synthetic::SPIKE_SEED,
        "rows": synthetic::SPIKE_ROWS, "cols": synthetic::SPIKE_COLS,
        "tile_rows": 256, "samples": samples["samples"], "error": samples["error"],
        "timer": samples["timer"], "grid": samples["grid"], "editor": samples["editor"],
        "mode": std::env::var("OPENROWS_SPIKE").unwrap_or_default(),
        "verdict": if status == 0 { "collected" } else { "invalid" },
        "stage_summary_ms": stage_summary,
        "method": method,
    });
    let bytes =
        serde_json::to_vec(&payload).map_err(|error| SpikeError::Failed(error.to_string()))?;
    std::fs::write(path.with_extension("partial"), bytes)
        .map_err(|error| SpikeError::Failed(error.to_string()))?;
    std::fs::rename(path.with_extension("partial"), path)
        .map_err(|error| SpikeError::Failed(error.to_string()))?;
    app.exit(status);
    drop(app);
    Ok(())
}

fn method(report: &SpikeReport) -> &'static str {
    if report.editor.is_some() {
        "WKWebView page clock per scripted keystroke: execCommand insertText or keydown dispatch, first docChanged update, first rAF after it, then a MessageChannel task posted from that rAF (after the rendering update, not GPU display); keystrokeToPaintMs = queue + input + frame, excluding only the wait from the update to the rAF frame start; fresh 5000-line document per configuration; no discarded samples"
    } else if report.grid.is_some() {
        "WKWebView rAF deltas and Performance API around complete canvas painters; four scenarios per option; virtual 100-column variant; no discarded frame samples"
    } else {
        "Page Performance API: invoke to bytes, post bytes to typed reply (decodeMs), request to typed columns (totalMs); workerDecodeMs uses worker clock and is informational; timer is min positive delta of 100000 consecutive page reads; worker cold start excluded; no cache hits"
    }
}

fn report_path() -> Result<PathBuf, SpikeError> {
    if !enabled() {
        return Err(SpikeError::Disabled);
    }
    std::env::var_os("OPENROWS_SPIKE_REPORT")
        .map(PathBuf::from)
        .ok_or(SpikeError::MissingPath)
}

fn summaries(samples: &[TileTiming]) -> serde_json::Value {
    type Stage = (&'static str, fn(&TileTiming) -> f64);
    let stages: [Stage; 4] = [
        ("invokeMs", |sample| sample.invoke_ms),
        ("decodeMs", |sample| sample.decode_ms),
        ("workerDecodeMs", |sample| sample.worker_decode_ms),
        ("totalMs", |sample| sample.total_ms),
    ];
    let mut output = serde_json::Map::new();
    for (name, value) in stages {
        let mut values: Vec<_> = samples.iter().map(value).collect();
        values.sort_by(f64::total_cmp);
        if !values.is_empty() {
            let mid = values.len() / 2;
            let median = values[mid].midpoint(values[(values.len() - 1) / 2]);
            let (sum, count) = values
                .iter()
                .fold((0.0, 0.0), |(sum, count), value| (sum + value, count + 1.0));
            output.insert(
                name.to_owned(),
                serde_json::json!({
                    "p50": median, "p95": values[(values.len() * 95).div_ceil(100) - 1],
                    "max": values.last(), "mean": sum / count,
                }),
            );
        }
    }
    serde_json::Value::Object(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_appends_single_lines() -> Result<(), Box<dyn std::error::Error>> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!(".spike-test-{}.json", std::process::id()));
        let progress = path.with_extension("progress");
        let result = (|| {
            append_progress(&path, "worker-created", "blob\nready")?;
            append_progress(&path, "finishing", "error\rmessage")?;
            let text = std::fs::read_to_string(&progress)?;
            assert_eq!(
                text,
                "worker-created: blob ready\nfinishing: error message\n"
            );
            Ok(())
        })();
        if progress.exists() {
            std::fs::remove_file(progress)?;
        }
        result
    }

    #[test]
    fn modes_are_explicit_and_editor_skips_synthetic_rows() {
        assert_eq!(SpikeMode::parse("ipc"), Some(SpikeMode::Ipc));
        assert_eq!(SpikeMode::parse("grid"), Some(SpikeMode::Grid));
        assert_eq!(SpikeMode::parse("editor"), Some(SpikeMode::Editor));
        assert_eq!(SpikeMode::parse("Editor"), None);
        assert_eq!(SpikeMode::parse(""), None);
        assert!(SpikeMode::Ipc.needs_rows() && SpikeMode::Grid.needs_rows());
        assert!(!SpikeMode::Editor.needs_rows());
    }

    #[test]
    fn summary_matches_median_and_nearest_rank() {
        let samples: Vec<_> = (1..=500)
            .map(|value| TileTiming {
                row_start: value,
                invoke_ms: f64::from(value),
                decode_ms: f64::from(value) / 2.0,
                worker_decode_ms: f64::from(value) / 4.0,
                total_ms: f64::from(value) * 2.0,
            })
            .collect();
        let result = summaries(&samples);
        assert_eq!(result["invokeMs"]["p50"], 250.5);
        assert_eq!(result["invokeMs"]["p95"], 475.0);
        assert_eq!(result["totalMs"]["max"], 1000.0);
        for (stage, mean) in [
            ("invokeMs", 250.5),
            ("decodeMs", 125.25),
            ("workerDecodeMs", 62.625),
            ("totalMs", 501.0),
        ] {
            assert_eq!(result[stage]["mean"], mean);
        }
        assert_eq!(summaries(&[]), serde_json::json!({}));
    }
}
