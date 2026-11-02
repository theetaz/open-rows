// SPDX-License-Identifier: Apache-2.0
//! Private metadata, the discovered catalogue and session-only tool grants.
//! Credential values never enter either file.
use crate::connections::{IpcError, IpcErrorKind};
use openrows_mcp::client::{
    ClientTool, Consent, ProjectServer, ProjectTrust, ServerConfig, TrustEntry,
};
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
};
use tokio::sync::oneshot;

const METADATA_LIMIT: usize = 65536;
const CATALOGUE_LIMIT: usize = 1 << 20;
pub(crate) struct State {
    pub path: PathBuf,
    catalogue: PathBuf,
    /// Hash-bound approvals of Project servers (`mcp-project-trust.json`).
    pub trust: ProjectTrust,
    pub inner: Mutex<Inner>,
}
/// One server declared by a loaded Project, with its device trust.
#[derive(Clone)]
pub(crate) struct ProjectEntry {
    /// Canonical Project root.
    pub root: String,
    /// Project display name.
    pub project: String,
    pub server: ProjectServer,
    /// Device key: keychain, catalogue and trust address.
    pub key: String,
    /// `None` until the user trusts this exact definition.
    pub trust: Option<TrustEntry>,
    /// A previous approval was removed because the definition changed.
    pub changed: bool,
}
impl ProjectEntry {
    /// Runnable configuration; only trusted entries have one.
    pub fn config(&self) -> Option<ServerConfig> {
        self.trust.as_ref().map(|trust| {
            self.server
                .effective(&self.key, &self.project, trust.enabled_tools.clone())
        })
    }
}
#[derive(Default)]
pub(crate) struct Inner {
    pub configs: Vec<ServerConfig>,
    pub tools: HashMap<String, Vec<ClientTool>>,
    pub consent: Consent,
    pub pending: HashMap<String, oneshot::Sender<bool>>,
    pub sequence: u64,
    pub revision: u64,
    /// Servers of every Project loaded this session; untrusted ones never start.
    pub projects: Vec<ProjectEntry>,
}
impl Inner {
    pub fn current(&self, revision: u64, config: &ServerConfig) -> bool {
        self.revision == revision && self.servers().contains(config)
    }
    /// User servers, then trusted Project servers with their enabled tools.
    pub fn servers(&self) -> Vec<ServerConfig> {
        let projects = self.projects.iter().filter_map(ProjectEntry::config);
        self.configs.iter().cloned().chain(projects).collect()
    }
    /// The loaded Project entry stored under `key`.
    pub fn project(&self, key: &str) -> Option<&ProjectEntry> {
        self.projects.iter().find(|entry| entry.key == key)
    }
}
impl State {
    pub fn new(path: PathBuf) -> Result<Self, IpcError> {
        let configs: Vec<ServerConfig> = match std::fs::read(&path) {
            Ok(bytes) if bytes.len() <= METADATA_LIMIT => {
                serde_json::from_slice(&bytes).map_err(|_| storage())?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
            _ => return Err(storage()),
        };
        validate(&configs)?;
        let catalogue = path.with_file_name("mcp-tools.json");
        let mut tools = load_catalogue(&catalogue);
        tools.retain(|id, _| configs.iter().any(|config| &config.id == id));
        Ok(Self {
            trust: ProjectTrust::new(&path.with_file_name("mcp-project-trust.json")),
            path,
            catalogue,
            inner: Mutex::new(Inner {
                configs,
                tools,
                ..Inner::default()
            }),
        })
    }
    pub fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn respond(&self, id: &str, allow: bool) -> Result<(), IpcError> {
        let send = self
            .lock()
            .pending
            .remove(id)
            .ok_or_else(|| super::failure(openrows_mcp::client::ClientError::Confirmation))?;
        send.send(allow)
            .map_err(|_| super::failure(openrows_mcp::client::ClientError::Cancelled))
    }
    /// Lock or settings change: forget grants and pending prompts, keep the catalogue.
    pub fn invalidate(&self) {
        let mut state = self.lock();
        state.revision = state.revision.wrapping_add(1);
        state.consent.clear();
        state.pending.clear();
    }
    /// Schemas were filtered with the previous credentials, so they go too.
    pub fn invalidate_credentials(&self, id: &str) {
        let mut state = self.lock();
        state.revision = state.revision.wrapping_add(1);
        state.consent.clear();
        state.pending.clear();
        if state.tools.remove(id).is_some() {
            self.persist_catalogue(&state);
        }
    }
    /// Record an explicit discovery made at `revision`; stale results are refused.
    pub fn store_tools(
        &self,
        config: &ServerConfig,
        revision: u64,
        tools: Vec<ClientTool>,
    ) -> Result<(), IpcError> {
        let mut state = self.lock();
        if !state.current(revision, config) {
            return Err(super::failure(openrows_mcp::client::ClientError::Cancelled));
        }
        if let Some(entry) = state.project(&config.id).cloned() {
            let kept = tools.clone();
            self.trust
                .edit(&entry.key, &entry.server, |trust| trust.tools = kept)
                .map_err(super::failure)?;
        }
        state.tools.insert(config.id.clone(), tools);
        self.persist_catalogue(&state);
        Ok(())
    }
    pub fn save(&self, configs: Vec<ServerConfig>) -> Result<(), IpcError> {
        validate(&configs)?;
        let bytes = serde_json::to_vec_pretty(&configs).map_err(|_| storage())?;
        if bytes.len() > METADATA_LIMIT {
            return Err(IpcError::invalid("MCP metadata exceeds limits"));
        }
        let mut state = self.lock();
        write_private(&self.path, &bytes).map_err(|_| storage())?;
        let unchanged: std::collections::HashSet<_> = configs
            .iter()
            .filter(|next| {
                state
                    .configs
                    .iter()
                    .any(|previous| previous.id == next.id && previous.endpoint == next.endpoint)
            })
            .map(|config| config.id.clone())
            .collect();
        let projects: std::collections::HashSet<_> = state
            .projects
            .iter()
            .map(|entry| entry.key.clone())
            .collect();
        state
            .tools
            .retain(|id, _| unchanged.contains(id) || projects.contains(id));
        state.configs = configs;
        let servers = state.servers();
        state.consent.retain(&servers);
        state.revision = state.revision.wrapping_add(1);
        state.pending.clear();
        self.persist_catalogue(&state);
        Ok(())
    }
    /// A catalogue that cannot be written is removed, so no stale schema survives restart.
    /// Project catalogues live with their trust record instead.
    pub(super) fn persist_catalogue(&self, state: &Inner) {
        let user: HashMap<_, _> = state
            .tools
            .iter()
            .filter(|(id, _)| state.configs.iter().any(|config| &config.id == *id))
            .collect();
        let written = serde_json::to_vec(&user)
            .ok()
            .filter(|bytes| bytes.len() <= CATALOGUE_LIMIT)
            .is_some_and(|bytes| write_private(&self.catalogue, &bytes).is_ok());
        if !written {
            let _ = std::fs::remove_file(&self.catalogue);
        }
    }
}
fn load_catalogue(path: &Path) -> HashMap<String, Vec<ClientTool>> {
    std::fs::read(path)
        .ok()
        .filter(|bytes| bytes.len() <= CATALOGUE_LIMIT)
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}
/// Owner-only atomic replacement.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temp = path.with_extension("tmp");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&temp)?.write_all(bytes)?;
    std::fs::rename(&temp, path)
}
fn validate(configs: &[ServerConfig]) -> Result<(), IpcError> {
    if configs.len() > 16 {
        return Err(IpcError::invalid("Too many MCP servers"));
    }
    let mut ids = std::collections::HashSet::new();
    for config in configs {
        config.validate().map_err(super::failure)?;
        if !ids.insert(&config.id) {
            return Err(IpcError::invalid("Duplicate MCP server"));
        }
    }
    Ok(())
}
fn storage() -> IpcError {
    IpcError::new(IpcErrorKind::Storage, "MCP client metadata unavailable")
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
