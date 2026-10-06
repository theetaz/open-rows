// SPDX-License-Identifier: Apache-2.0
//! Spike-only grid samples; no benchmark instrumentation in normal mode.
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GridRun {
    option: String,
    scenario: String,
    cols: u32,
    frame_ms: Vec<f64>,
    frame_breakdown: Vec<GridFrame>,
    longest_frames: Vec<GridFrame>,
    paint_ms: Vec<f64>,
    blank_cell_frames: u32,
    tiles_requested: u32,
    cache_hits: u32,
    duration_ms: f64,
    start_row: u32,
    end_row: u32,
    end_column: u32,
    paint_method: String,
    max_scroll_error_px: f64,
}

/// Per-rAF interval diagnostics. Timings may overlap; worker CPU is informational.
#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GridFrame {
    index: u32,
    frame_ms: f64,
    step: String,
    scroll_handler_ms: f64,
    react_commit_ms: f64,
    canvas_resize_ms: f64,
    canvas_resize_events: u32,
    prepare_ms: f64,
    paint_ms: f64,
    tile_apply_ms: f64,
    tile_convert_ms: f64,
    worker_convert_ms: f64,
    tile_allocations: u32,
    array_allocations: u32,
    cell_allocations: u32,
    driver_ms: f64,
}
