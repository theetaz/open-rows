// SPDX-License-Identifier: Apache-2.0
//! `schema_diagram` and `diagram_save`.
use std::path::PathBuf;

use tauri::State;
use tauri_plugin_dialog::DialogExt as _;

use super::MAX_TABLES;
use super::dto::{DiagramFileDto, DiagramTarget, SchemaDiagramDto};
use crate::connections::catalog::read_diagram;
use crate::connections::{Connections, IpcError};
use crate::queries::files::{choose, file_stem, write_atomic};

/// Largest export written (a PNG of a very large diagram stays far below).
const MAX_EXPORT_BYTES: usize = 256 * 1024 * 1024;

/// Tables, columns and foreign keys of a schema, read on the metadata session.
#[tauri::command]
#[specta::specta]
pub(crate) async fn schema_diagram(
    state: State<'_, Connections>,
    target: DiagramTarget,
) -> Result<SchemaDiagramDto, IpcError> {
    let diagram = read_diagram(
        &state,
        (&target.profile_id, &target.database),
        target.schema,
        MAX_TABLES,
    )
    .await?;
    Ok(SchemaDiagramDto::new(diagram, MAX_TABLES))
}

/// Ask where to save an exported diagram and write it; returns the written
/// path, or `null` when the panel is cancelled.
#[tauri::command]
#[specta::specta]
pub(crate) async fn diagram_save(
    window: tauri::WebviewWindow,
    name: String,
    file: DiagramFileDto,
) -> Result<Option<String>, IpcError> {
    if file.contents().len() > MAX_EXPORT_BYTES {
        return Err(IpcError::invalid("The diagram export is too large"));
    }
    let (extension, filter) = file.kind();
    let panel = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Export ER Diagram")
        .add_filter(filter, &[extension])
        .set_file_name(format!("{}.{extension}", file_stem(&name, "diagram")));
    let Some(path) = choose(panel, true).await? else {
        return Ok(None);
    };
    let path = with_extension(path, extension);
    write_atomic(&path, file.contents())?;
    Ok(Some(path.display().to_string()))
}

/// `path`, with `.extension` appended unless it already ends in it.
pub(crate) fn with_extension(mut path: PathBuf, extension: &str) -> PathBuf {
    let matches = path
        .extension()
        .is_some_and(|current| current.eq_ignore_ascii_case(extension));
    if !matches {
        path.as_mut_os_string().push(format!(".{extension}"));
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_get_their_extension_once() {
        let png = with_extension(PathBuf::from("/tmp/shop.png"), "png");
        assert_eq!(png, PathBuf::from("/tmp/shop.png"));
        let bare = with_extension(PathBuf::from("/tmp/shop"), "svg");
        assert_eq!(bare, PathBuf::from("/tmp/shop.svg"));
    }

    #[test]
    fn files_carry_their_bytes_and_kind() -> Result<(), serde_json::Error> {
        let svg: DiagramFileDto =
            serde_json::from_value(serde_json::json!({ "format": "svg", "text": "<svg/>" }))?;
        assert_eq!(
            (svg.kind().0, svg.contents()),
            ("svg", b"<svg/>".as_slice())
        );
        let png: DiagramFileDto =
            serde_json::from_value(serde_json::json!({ "format": "png", "bytes": [137, 80] }))?;
        assert_eq!(
            (png.kind().0, png.contents()),
            ("png", [137, 80].as_slice())
        );
        Ok(())
    }
}
