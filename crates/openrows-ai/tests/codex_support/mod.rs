// SPDX-License-Identifier: Apache-2.0
//! Fake `codex app-server` fixtures in a private app-data root.
#![allow(dead_code, reason = "shared by the status and conversation suites")]
use openrows_ai::{ProviderConfig, ProviderKind, RequestOptions, codex::Bridge};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

pub const FAKE: &str = env!("CARGO_BIN_EXE_fake-codex");

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub config: ProviderConfig,
    pub bridge: Bridge,
}
impl Fixture {
    /// A Codex provider pointing at the fake, playing `scenario`.
    pub fn new(scenario: &str) -> Result<Self> {
        Self::with_executable(scenario, FAKE)
    }
    pub fn with_executable(scenario: &str, executable: &str) -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let root = dir.path().join("codex");
        std::fs::create_dir_all(root.join("home"))?;
        std::fs::write(root.join("home/scenario"), scenario)?;
        let config = config(executable);
        let bridge = Bridge::new(&config, root)?;
        Ok(Self {
            dir,
            config,
            bridge,
        })
    }
    pub fn root(&self) -> PathBuf {
        self.dir.path().join("codex")
    }
    /// Report a different `codex --version` output.
    pub fn version(&self, output: &str) -> Result {
        Ok(std::fs::write(self.root().join("home/version"), output)?)
    }
    /// What the fake recorded under `name`.
    pub fn recorded(&self, name: &str) -> Result<Value> {
        let text = std::fs::read_to_string(self.root().join(format!("home/{name}.json")))?;
        Ok(serde_json::from_str(&text)?)
    }
    pub fn launched(&self) -> bool {
        self.root().join("home/launch.json").exists()
    }
}
pub fn config(executable: &str) -> ProviderConfig {
    ProviderConfig {
        id: "fixture".into(),
        name: "Codex".into(),
        kind: ProviderKind::Codex,
        base_url: executable.into(),
        allow_insecure_remote: false,
        secret_ref: String::new(),
    }
}
pub fn options(timeout: Duration) -> RequestOptions {
    RequestOptions {
        timeout,
        ..RequestOptions::default()
    }
}
/// Whether `pid` is a live (non-zombie) process.
pub fn alive(pid: u64) -> bool {
    std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .map(|output| {
            let state = String::from_utf8_lossy(&output.stdout);
            let state = state.trim();
            !state.is_empty() && !state.starts_with('Z')
        })
        .unwrap_or(false)
}
/// Busy-wait (no sleeping) until every PID has exited, within five seconds.
pub fn reaped(pids: &[u64]) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if pids.iter().all(|pid| !alive(*pid)) {
            return true;
        }
        std::thread::yield_now();
    }
    false
}
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
