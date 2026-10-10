// SPDX-License-Identifier: Apache-2.0
//! Installed resolution uses only the directory of the running application binary.
use crate::HostFailure;
use std::path::{Path, PathBuf};

/// Unsuffixed name installed by Tauri beside the application executable.
pub const HOST_BINARY_NAME: &str = if cfg!(windows) {
    "openrows-driver-host.exe"
} else {
    "openrows-driver-host"
};

/// Resolve the sidecar beside an absolute app executable; never consult PATH/cwd/env.
/// # Errors
/// Missing/non-executable helper is Spawn; wrong format/architecture is Incompatible.
pub fn resolve_installed(app_executable: &Path) -> Result<PathBuf, HostFailure> {
    if !app_executable.is_absolute() {
        return Err(HostFailure::Spawn);
    }
    let helper = app_executable
        .parent()
        .ok_or(HostFailure::Spawn)?
        .join(HOST_BINARY_NAME);
    validate(&helper)?;
    Ok(helper)
}

pub(crate) fn validate(helper: &Path) -> Result<(), HostFailure> {
    if !helper.is_absolute() {
        return Err(HostFailure::Spawn);
    }
    let metadata = helper.metadata().map_err(|_| HostFailure::Spawn)?;
    if !metadata.is_file() {
        return Err(HostFailure::Spawn);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(HostFailure::Spawn);
        }
    }
    let mut binary = std::fs::File::open(helper).map_err(|_| HostFailure::Spawn)?;
    crate::binary::validate(&mut binary)
}
