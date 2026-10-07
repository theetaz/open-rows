// SPDX-License-Identifier: Apache-2.0
//! Spike-only editor keystroke samples (M0-S5); nothing is recorded in normal mode.
use serde::{Deserialize, Serialize};
use specta::Type;

/// One editor configuration measured on a freshly mounted 5 000-line document.
#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorRun {
    config: String,
    lines: u32,
    document_chars: u32,
    refresh_interval_ms: f64,
    duration_ms: f64,
    samples: Vec<KeystrokeSample>,
}

/// Page-clock phases of one keystroke; `keystrokeToPaintMs` excludes only `vsyncWaitMs`.
#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KeystrokeSample {
    index: u32,
    position: String,
    kind: String,
    key: String,
    queue_ms: f64,
    input_ms: f64,
    vsync_wait_ms: f64,
    frame_ms: f64,
    wall_ms: f64,
    keystroke_to_paint_ms: f64,
}
