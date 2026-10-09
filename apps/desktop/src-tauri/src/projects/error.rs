// SPDX-License-Identifier: Apache-2.0
//! Why a project operation failed; converted to [`IpcError`] at the boundary.
use std::path::Path;

use crate::connections::{IpcError, IpcErrorKind};

/// A project command failure, safe to show to the user.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ProjectError {
    /// The project folder was given as a relative path.
    #[error("The project folder must be an absolute path")]
    NotAbsolute,
    /// The folder has no `.openrows/project.yaml`.
    #[error("{0} is not an OpenRows project (no .openrows/project.yaml)")]
    NotProject(String),
    /// A path that would leave the project folder (`..`, absolute, a symlink).
    #[error("{0} is outside the project folder")]
    Escapes(String),
    /// A path outside the project's query folders, or the folder itself.
    #[error("{0} is not inside a query folder of the project")]
    OutsideQueries(String),
    /// A file or folder name that cannot be used.
    #[error("{0}")]
    InvalidName(&'static str),
    /// `project.yaml` cannot be parsed or breaks a rule.
    #[error("project.yaml: {0}")]
    Manifest(String),
    /// The target of a create, rename, move or copy already exists.
    #[error("{0} already exists")]
    Exists(String),
    /// A folder cannot move into itself or one of its subfolders.
    #[error("A folder cannot be moved into itself")]
    IntoItself,
    /// The file system refused.
    #[error("{path}: {message}")]
    Io {
        /// The path the operation used.
        path: String,
        /// The operating system's message.
        message: String,
    },
}

impl ProjectError {
    /// An I/O failure on `path`.
    pub(crate) fn io(path: &Path, error: &std::io::Error) -> Self {
        Self::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        }
    }
}

impl From<ProjectError> for IpcError {
    fn from(error: ProjectError) -> Self {
        let kind = match error {
            ProjectError::NotProject(_) => IpcErrorKind::NotFound,
            ProjectError::Io { .. } => IpcErrorKind::Storage,
            _ => IpcErrorKind::Invalid,
        };
        Self::new(kind, error.to_string())
    }
}
