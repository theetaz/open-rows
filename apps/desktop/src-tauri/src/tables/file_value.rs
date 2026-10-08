// SPDX-License-Identifier: Apache-2.0
//! "Add a file…" for a staged field: Rust shows the native open panel and reads the
//! picked file, so the webview never holds file-system permissions or chooses paths.
use crate::connections::{IpcError, IpcErrorKind};
use serde::Deserialize;
use specta::Type;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::Path;
use tauri_plugin_dialog::DialogExt as _;

/// Largest file a field accepts (16 MiB), checked while reading.
const FILE_LIMIT: u64 = 16 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// How the picked file becomes `PostgreSQL` text input.
#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum FileEncodingDto {
    /// UTF-8 text for text, varchar, json and xml columns.
    Text,
    /// `\x` hex input for bytea columns.
    Hex,
}

/// Why a picked file cannot become a field value.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FileValueError {
    /// The file exceeds the size cap.
    #[error("The file is larger than {limit_mib} MiB")]
    TooLarge {
        /// The cap in MiB.
        limit_mib: u64,
    },
    /// Text columns need valid UTF-8 without NUL characters.
    #[error(
        "The file is not UTF-8 text without NUL characters; use a bytea column for binary data"
    )]
    NotText,
    /// The file could not be opened or read.
    #[error("The file could not be read: {0}")]
    Unreadable(String),
}

impl From<FileValueError> for IpcError {
    fn from(error: FileValueError) -> Self {
        let kind = match error {
            FileValueError::Unreadable(_) => IpcErrorKind::Storage,
            FileValueError::TooLarge { .. } | FileValueError::NotText => IpcErrorKind::Invalid,
        };
        Self::new(kind, error.to_string())
    }
}

/// Read at most `limit` bytes of `path` as a field value in `encoding`.
///
/// # Errors
/// Oversized files, non-UTF-8 or NUL text, and I/O failures.
pub(crate) fn read_value(
    path: &Path,
    encoding: FileEncodingDto,
    limit: u64,
) -> Result<String, FileValueError> {
    let unreadable = |error: std::io::Error| FileValueError::Unreadable(error.kind().to_string());
    let file = std::fs::File::open(path).map_err(unreadable)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > limit) {
        return Err(FileValueError::TooLarge {
            limit_mib: limit.div_ceil(MIB),
        });
    }
    match encoding {
        FileEncodingDto::Text => String::from_utf8(bytes)
            .ok()
            .filter(|text| !text.contains('\0'))
            .ok_or(FileValueError::NotText),
        FileEncodingDto::Hex => Ok(bytea_hex(&bytes)),
    }
}

/// `PostgreSQL` bytea hex input, e.g. `\x00ff`.
fn bytea_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(2 + bytes.len() * 2);
    text.push_str("\\x");
    for byte in bytes {
        // Writing to a String cannot fail.
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// Pick a file in a native open panel attached to the window and return its contents
/// as a field value; `null` when the panel is cancelled. Nothing is staged here.
#[tauri::command]
#[specta::specta]
pub(crate) async fn table_edit_file(
    window: tauri::WebviewWindow,
    encoding: FileEncodingDto,
) -> Result<Option<String>, IpcError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Add a file")
        .pick_file(move |picked| {
            // The receiver only disappears when the command was dropped.
            let _ = sender.send(picked);
        });
    let picked = receiver
        .await
        .map_err(|_| IpcError::new(IpcErrorKind::Internal, "The file panel closed unexpectedly"))?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|error| IpcError::invalid(error.to_string()))?;
    tauri::async_runtime::spawn_blocking(move || read_value(&path, encoding, FILE_LIMIT))
        .await
        .map_err(|error| IpcError::new(IpcErrorKind::Internal, error.to_string()))?
        .map(Some)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn file(bytes: &[u8]) -> Result<tempfile::NamedTempFile, std::io::Error> {
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(bytes)?;
        Ok(file)
    }

    #[test]
    fn text_files_become_exact_utf8_text() -> Result<(), Box<dyn std::error::Error>> {
        let picked = file("{\"a\": 1}\nĀ".as_bytes())?;
        assert_eq!(
            read_value(picked.path(), FileEncodingDto::Text, FILE_LIMIT)?,
            "{\"a\": 1}\nĀ"
        );
        Ok(())
    }

    #[test]
    fn binary_files_become_bytea_hex() -> Result<(), Box<dyn std::error::Error>> {
        let picked = file(&[0x00, 0xff, 0x7f, b'\n'])?;
        assert_eq!(
            read_value(picked.path(), FileEncodingDto::Hex, FILE_LIMIT)?,
            "\\x00ff7f0a"
        );
        let empty = file(&[])?;
        assert_eq!(
            read_value(empty.path(), FileEncodingDto::Hex, FILE_LIMIT)?,
            "\\x"
        );
        Ok(())
    }

    #[test]
    fn non_utf8_and_nul_text_are_refused() -> Result<(), Box<dyn std::error::Error>> {
        let invalid = file(&[0xc3, 0x28])?;
        assert_eq!(
            read_value(invalid.path(), FileEncodingDto::Text, FILE_LIMIT),
            Err(FileValueError::NotText)
        );
        let nul = file(b"a\0b")?;
        assert_eq!(
            read_value(nul.path(), FileEncodingDto::Text, FILE_LIMIT),
            Err(FileValueError::NotText)
        );
        Ok(())
    }

    #[test]
    fn files_over_the_cap_are_refused_with_a_typed_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let limit = 2 * MIB;
        let exact = file(&vec![b'a'; 2 * 1024 * 1024])?;
        assert_eq!(
            read_value(exact.path(), FileEncodingDto::Text, limit)?.len(),
            2 * 1024 * 1024
        );
        let over = file(&vec![0_u8; 2 * 1024 * 1024 + 1])?;
        let error = read_value(over.path(), FileEncodingDto::Hex, limit);
        assert_eq!(error, Err(FileValueError::TooLarge { limit_mib: 2 }));
        let ipc = IpcError::from(FileValueError::TooLarge { limit_mib: 16 });
        assert_eq!(ipc.kind, IpcErrorKind::Invalid);
        assert_eq!(ipc.message, "The file is larger than 16 MiB");
        Ok(())
    }

    #[test]
    fn missing_files_are_storage_errors() {
        let error = read_value(
            Path::new("/nonexistent/openrows-test-file"),
            FileEncodingDto::Text,
            FILE_LIMIT,
        );
        assert!(matches!(error, Err(FileValueError::Unreadable(_))));
        assert!(
            error
                .map_err(IpcError::from)
                .is_err_and(|error| error.kind == IpcErrorKind::Storage)
        );
    }
}
