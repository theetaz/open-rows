// SPDX-License-Identifier: Apache-2.0
//! Cloud tool discovery uses the same caller-supplied install directories as dump tools.
use super::{HelperError, scrub};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncReadExt as _, process::Command};

/// Every cloud tool shown alongside database client tools in Settings.
pub const TOOLS: [(&str, &str); 6] = [
    ("aws", "aws"),
    ("sessionManagerPlugin", "session-manager-plugin"),
    ("kubectl", "kubectl"),
    ("cloudSqlProxy", "cloud-sql-proxy"),
    ("alloydbAuthProxy", "alloydb-auth-proxy"),
    ("cloudflared", "cloudflared"),
];

/// Resolved path and detected version.
#[derive(Clone, Debug)]
pub struct HelperTool {
    /// Executable path.
    pub path: PathBuf,
    /// Scrubbed version output.
    pub version: String,
}

/// Find and probe one tool, respecting an explicit override without fallback.
///
/// # Errors
/// Missing tool, invalid override, or failed/slow version probe.
pub async fn discover(
    tool: &str,
    override_path: Option<&Path>,
    dirs: &[PathBuf],
) -> Result<HelperTool, HelperError> {
    let paths = override_path.map_or_else(
        || {
            dirs.iter()
                .map(|dir| dir.join(format!("{tool}{}", std::env::consts::EXE_SUFFIX)))
                .collect::<Vec<_>>()
        },
        |path| vec![path.to_path_buf()],
    );
    for path in paths {
        if executable(&path) {
            return version(tool, path).await;
        }
    }
    Err(HelperError::NotFound { tool: tool.into() })
}
fn executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}
async fn version(tool: &str, path: PathBuf) -> Result<HelperTool, HelperError> {
    let args = if tool == "kubectl" {
        vec!["version", "--client", "--output=json"]
    } else {
        vec!["--version"]
    };
    let mut command = Command::new(&path);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let failure = |message: String| HelperError::Network {
        tool: tool.into(),
        message,
        stderr: String::new(),
        hint: "Check the executable path in Settings ▸ Tools.".into(),
    };
    let mut child = command
        .spawn()
        .map_err(|error| failure(error.to_string()))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        let out = read(stdout);
        let err = read(stderr);
        let (out, err, status) = tokio::join!(out, err, child.wait());
        let status = status.map_err(|error| failure(error.to_string()))?;
        let text = format!("{} {}", out?, err?);
        if !status.success() || text.trim().is_empty() {
            return Err(failure("Version probe failed".into()));
        }
        Ok(HelperTool {
            path,
            version: text
                .lines()
                .map(scrub)
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_owned(),
        })
    })
    .await;
    if let Ok(result) = result {
        result
    } else {
        let _ = child.kill().await;
        Err(failure("Version probe timed out".into()))
    }
}
async fn read(pipe: Option<impl tokio::io::AsyncRead + Unpin>) -> Result<String, HelperError> {
    let mut bytes = Vec::new();
    if let Some(pipe) = pipe {
        pipe.take(8192)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| HelperError::Invalid(format!("Cannot read tool version: {error}")))?;
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
