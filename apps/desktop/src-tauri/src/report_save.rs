// SPDX-License-Identifier: Apache-2.0
//! `report_save`: write an exported Monitor report or chart (FR-ADMIN-08,
//! M25-08) to a file chosen in the native save panel.
//!
//! The webview builds the report from data it already shows; this command
//! only asks where to save it and writes the bytes atomically.
use serde::Deserialize;
use specta::Type;
use tauri_plugin_dialog::DialogExt as _;

use crate::connections::IpcError;
use crate::diagram::commands::with_extension;
use crate::queries::files::{choose, file_stem, write_atomic};

/// Largest report written; a 2× PNG of a report chart stays far below.
const MAX_REPORT_BYTES: usize = 64 * 1024 * 1024;

/// An exported report document or chart image.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "format")]
pub(crate) enum ReportFileDto {
    Markdown { text: String },
    Html { text: String },
    Json { text: String },
    Csv { text: String },
    Svg { text: String },
    Png { bytes: Vec<u8> },
}

impl ReportFileDto {
    /// File extension and the save panel's filter name.
    fn kind(&self) -> (&'static str, &'static str) {
        match self {
            Self::Markdown { .. } => ("md", "Markdown"),
            Self::Html { .. } => ("html", "HTML document"),
            Self::Json { .. } => ("json", "JSON"),
            Self::Csv { .. } => ("csv", "CSV"),
            Self::Svg { .. } => ("svg", "SVG image"),
            Self::Png { .. } => ("png", "PNG image"),
        }
    }

    fn contents(&self) -> &[u8] {
        match self {
            Self::Markdown { text }
            | Self::Html { text }
            | Self::Json { text }
            | Self::Csv { text }
            | Self::Svg { text } => text.as_bytes(),
            Self::Png { bytes } => bytes,
        }
    }
}

/// Ask where to save an exported report and write it; returns the written
/// path, or `null` when the panel is cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn report_save(
    window: tauri::WebviewWindow,
    name: String,
    file: ReportFileDto,
) -> Result<Option<String>, IpcError> {
    if file.contents().len() > MAX_REPORT_BYTES {
        return Err(IpcError::invalid("The report export is too large"));
    }
    let (extension, filter) = file.kind();
    let panel = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Export Report")
        .add_filter(filter, &[extension])
        .set_file_name(format!("{}.{extension}", file_stem(&name, "report")));
    let Some(path) = choose(panel, true).await? else {
        return Ok(None);
    };
    let path = with_extension(path, extension);
    write_atomic(&path, file.contents())?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_files_carry_their_bytes_and_extension() -> Result<(), serde_json::Error> {
        let html: ReportFileDto =
            serde_json::from_value(serde_json::json!({ "format": "html", "text": "<p>x</p>" }))?;
        assert_eq!(
            (html.kind().0, html.contents()),
            ("html", b"<p>x</p>".as_slice())
        );
        let markdown: ReportFileDto =
            serde_json::from_value(serde_json::json!({ "format": "markdown", "text": "# r" }))?;
        assert_eq!(markdown.kind().0, "md");
        let png: ReportFileDto =
            serde_json::from_value(serde_json::json!({ "format": "png", "bytes": [137, 80] }))?;
        assert_eq!(
            (png.kind().0, png.contents()),
            ("png", [137, 80].as_slice())
        );
        Ok(())
    }
}
