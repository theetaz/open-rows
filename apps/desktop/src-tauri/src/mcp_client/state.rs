// SPDX-License-Identifier: Apache-2.0
//! Private metadata and session-only tool grants; credential values never enter this file.
use crate::connections::{IpcError, IpcErrorKind};
use openrows_mcp::client::{ClientTool, Consent, ServerConfig};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, MutexGuard, PoisonError},
};
use tokio::sync::oneshot;

pub(crate) struct State {
    pub path: PathBuf,
    pub inner: Mutex<Inner>,
}
#[derive(Default)]
pub(crate) struct Inner {
    pub configs: Vec<ServerConfig>,
    pub tools: HashMap<String, Vec<ClientTool>>,
    pub consent: Consent,
    pub pending: HashMap<String, oneshot::Sender<bool>>,
    pub sequence: u64,
    pub revision: u64,
}
impl State {
    pub fn new(path: PathBuf) -> Result<Self, IpcError> {
        let configs: Vec<ServerConfig> = match std::fs::read(&path) {
            Ok(bytes) if bytes.len() <= 65536 => {
                serde_json::from_slice(&bytes).map_err(|_| storage())?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
            _ => return Err(storage()),
        };
        validate(&configs)?;
        Ok(Self {
            path,
            inner: Mutex::new(Inner {
                configs,
                ..Inner::default()
            }),
        })
    }
    pub fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
    pub fn invalidate(&self) {
        let mut state = self.lock();
        state.revision = state.revision.wrapping_add(1);
        state.consent.clear();
        state.pending.clear();
    }
    pub fn invalidate_credentials(&self, id: &str) {
        let mut state = self.lock();
        state.revision = state.revision.wrapping_add(1);
        state.consent.clear();
        state.pending.clear();
        state.tools.remove(id);
    }
    pub fn save(&self, configs: Vec<ServerConfig>) -> Result<(), IpcError> {
        validate(&configs)?;
        let bytes = serde_json::to_vec_pretty(&configs).map_err(|_| storage())?;
        if bytes.len() > 65536 {
            return Err(IpcError::invalid("MCP metadata exceeds limits"));
        }
        let mut state = self.lock();
        let temp = self.path.with_extension("tmp");
        let mut file = std::fs::OpenOptions::new();
        file.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            file.mode(0o600);
        }
        use std::io::Write;
        file.open(&temp)
            .and_then(|mut file| file.write_all(&bytes))
            .map_err(|_| storage())?;
        std::fs::rename(&temp, &self.path).map_err(|_| storage())?;
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
        state.tools.retain(|id, _| unchanged.contains(id));
        state.configs = configs;
        state.revision = state.revision.wrapping_add(1);
        state.consent.clear();
        state.pending.clear();
        Ok(())
    }
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
mod tests {
    use super::*;
    fn config() -> ServerConfig {
        ServerConfig {
            id: "fixture".into(),
            name: "Fixture".into(),
            endpoint: openrows_mcp::client::Endpoint::Http {
                url: "http://127.0.0.1:1/mcp".into(),
            },
            enabled_tools: vec!["lookup".into()],
        }
    }
    #[test]
    fn public_metadata_round_trips_without_credential_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("mcp-servers.json");
        let state = State::new(path.clone())?;
        state.save(vec![config()])?;
        let restored = State::new(path.clone())?;
        assert!(restored.lock().configs == vec![config()]);
        let text = std::fs::read_to_string(&path)?;
        assert!(!text.contains("headers") && !text.contains("env"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path)?.permissions().mode() & 0o777,
                0o600
            );
        }
        assert!(!path.with_extension("tmp").exists());
        Ok(())
    }
    #[test]
    fn saving_disabled_tools_revokes_grants_and_drops_pending_prompts()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let state = State::new(dir.path().join("mcp-servers.json"))?;
        let saved = config();
        let tool = ClientTool {
            name: "lookup".into(),
            schema: serde_json::json!({"type":"object"}),
        };
        state.save(vec![saved.clone()])?;
        let (send, receive) = oneshot::channel();
        {
            let mut inner = state.lock();
            inner.consent.confirm(&saved, &tool)?;
            inner.pending.insert("pending".into(), send);
            inner.tools.insert(saved.id.clone(), vec![tool.clone()]);
        }
        let disabled = ServerConfig {
            enabled_tools: vec![],
            ..saved.clone()
        };
        state.save(vec![disabled.clone()])?;
        assert!(receive.blocking_recv().is_err());
        assert!(state.lock().consent.check(&saved, &tool).is_err());
        assert!(state.lock().consent.check(&disabled, &tool).is_err());
        assert_eq!(state.lock().tools[&saved.id].len(), 1);
        state.invalidate();
        assert!(state.lock().pending.is_empty());
        Ok(())
    }
    #[test]
    fn changed_endpoints_discard_catalogues_and_invalid_config_never_writes()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("mcp-servers.json");
        let state = State::new(path.clone())?;
        let saved = config();
        state.save(vec![saved.clone()])?;
        state.lock().tools.insert(saved.id.clone(), vec![]);
        let changed = ServerConfig {
            endpoint: openrows_mcp::client::Endpoint::Http {
                url: "http://localhost:2/mcp".into(),
            },
            ..saved.clone()
        };
        state.save(vec![changed])?;
        assert!(state.lock().tools.is_empty());
        let before = std::fs::read(&path)?;
        assert!(state.save(vec![saved.clone(), saved]).is_err());
        assert_eq!(std::fs::read(path)?, before);
        Ok(())
    }
    #[test]
    fn credential_changes_discard_schemas_filtered_with_previous_secrets()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let state = State::new(dir.path().join("mcp-servers.json"))?;
        let saved = config();
        let tool = ClientTool {
            name: "lookup".into(),
            schema: serde_json::json!({"type":"object"}),
        };
        state.save(vec![saved.clone()])?;
        state
            .lock()
            .tools
            .insert("other".into(), vec![tool.clone()]);
        // The second phase models discovery/consent during a keychain prompt.
        for _ in 0..2 {
            let (send, receive) = oneshot::channel();
            let revision = {
                let mut inner = state.lock();
                inner.consent.confirm(&saved, &tool)?;
                inner.tools.insert(saved.id.clone(), vec![tool.clone()]);
                inner.pending.insert("pending".into(), send);
                inner.revision
            };
            state.invalidate_credentials(&saved.id);
            assert!(receive.blocking_recv().is_err());
            let inner = state.lock();
            assert_ne!(inner.revision, revision);
            assert!(!inner.tools.contains_key(&saved.id));
            assert!(inner.tools.contains_key("other"));
            assert!(inner.consent.check(&saved, &tool).is_err());
        }
        Ok(())
    }
}
