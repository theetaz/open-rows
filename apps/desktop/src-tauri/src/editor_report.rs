// SPDX-License-Identifier: Apache-2.0
//! Spike-only editor keystroke (M0-S5) and completion popup (M1-21) samples; nothing is recorded in normal mode.
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

/// Completion popup latency on the 5 000-line document with a warm 50 000-object catalog (M1-21).
#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompletionRun {
    objects: u32,
    activate_delay_ms: f64,
    refresh_interval_ms: f64,
    samples: Vec<CompletionSample>,
}

/// Page-clock phases of one popup: `popupMs` = `openMs` (typed trigger to popup insertion) + `paintMs`.
#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompletionSample {
    index: u32,
    kind: String,
    key: String,
    options: u32,
    open_ms: f64,
    paint_ms: f64,
    popup_ms: f64,
}
