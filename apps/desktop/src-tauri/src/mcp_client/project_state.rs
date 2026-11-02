// SPDX-License-Identifier: Apache-2.0
//! Project MCP servers in the session state (docs/23 §3.2). Declared servers
//! are registered when their Project loads; only definitions the user trusted
//! on this device become runnable, and a changed definition loses its trust,
//! enabled tools, catalogue, grants and keychain values.
use super::state::{Inner, ProjectEntry, State};
use crate::connections::IpcError;
use openrows_mcp::client::{
    ClientError, Credentials, ProjectServer, TrustEntry, TrustState, project_key,
};

/// Longest Project name shown in a server label.
const PROJECT_LABEL: usize = 40;

impl State {
    /// Replace the servers of the Project at `root`; nothing starts. Returns the
    /// keys whose keychain values must be deleted because their definition changed.
    pub fn register_project(
        &self,
        root: &str,
        project: &str,
        servers: &[ProjectServer],
    ) -> Vec<String> {
        let label: String = project.chars().take(PROJECT_LABEL).collect();
        let mut fresh: Vec<ProjectEntry> = servers
            .iter()
            .map(|server| self.entry(root, &label, server))
            .collect();
        let mut inner = self.lock();
        let before = inner.servers();
        for entry in &mut fresh {
            // A change seen earlier stays visible until the user reviews it again.
            entry.changed |=
                entry.trust.is_none() && inner.project(&entry.key).is_some_and(|old| old.changed);
        }
        let stale = fresh
            .iter()
            .filter(|entry| entry.changed)
            .map(|entry| entry.key.clone())
            .collect();
        let old: Vec<String> = inner
            .projects
            .iter()
            .filter(|entry| entry.root == root)
            .map(|entry| entry.key.clone())
            .collect();
        for key in &old {
            inner.tools.remove(key);
        }
        inner.projects.retain(|entry| entry.root != root);
        for entry in fresh {
            if let Some(trust) = &entry.trust {
                inner.tools.insert(entry.key.clone(), trust.tools.clone());
            }
            inner.projects.push(entry);
        }
        refresh(&mut inner, &before);
        stale
    }

    /// Trust the definition the user reviewed (`digest`); refused if it changed.
    pub fn trust_project(&self, key: &str, digest: &str) -> Result<(), IpcError> {
        let entry = self.loaded(key)?;
        self.trust
            .trust(key, &entry.server, digest)
            .map_err(super::failure)?;
        self.apply(
            key,
            Some(TrustEntry {
                digest: digest.to_owned(),
                ..TrustEntry::default()
            }),
        );
        Ok(())
    }

    /// Withdraw trust: the server stops being offered and its catalogue is dropped.
    pub fn revoke_project(&self, key: &str) -> Result<(), IpcError> {
        self.loaded(key)?;
        self.trust.revoke(key).map_err(super::failure)?;
        self.apply(key, None);
        Ok(())
    }

    /// Enable exactly `tools` of a trusted server; all others are disabled.
    pub fn enable_project_tools(&self, key: &str, tools: Vec<String>) -> Result<(), IpcError> {
        let entry = self.loaded(key)?;
        entry
            .server
            .effective(key, &entry.project, tools.clone())
            .validate()
            .map_err(super::failure)?;
        let trust = self
            .trust
            .edit(key, &entry.server, |trust| trust.enabled_tools = tools)
            .map_err(super::failure)?;
        self.apply(key, Some(trust));
        Ok(())
    }

    /// Credentials changed: its schemas were filtered with the old values.
    pub fn forget_project_catalogue(&self, key: &str) -> Result<(), IpcError> {
        let entry = self.loaded(key)?;
        let trust = self
            .trust
            .edit(key, &entry.server, |trust| trust.tools.clear())
            .map_err(super::failure)?;
        self.apply(key, Some(trust));
        self.invalidate_credentials(key);
        Ok(())
    }

    /// A loaded Project server by device key.
    pub fn loaded(&self, key: &str) -> Result<ProjectEntry, IpcError> {
        self.lock()
            .project(key)
            .cloned()
            .ok_or_else(|| super::failure(ClientError::Invalid))
    }

    fn entry(&self, root: &str, project: &str, server: &ProjectServer) -> ProjectEntry {
        let key = project_key(root, &server.id);
        // An unreadable trust file fails closed: the server stays untrusted.
        let state = self
            .trust
            .state(&key, server)
            .unwrap_or(TrustState::Untrusted);
        let (trust, changed) = match state {
            TrustState::Trusted(entry) => (Some(entry), false),
            TrustState::Changed => (None, true),
            TrustState::Untrusted => (None, false),
        };
        ProjectEntry {
            root: root.to_owned(),
            project: project.to_owned(),
            server: server.clone(),
            key,
            trust,
            changed,
        }
    }

    fn apply(&self, key: &str, trust: Option<TrustEntry>) {
        let mut inner = self.lock();
        let before = inner.servers();
        match &trust {
            Some(trust) => {
                inner.tools.insert(key.to_owned(), trust.tools.clone());
            }
            None => {
                inner.tools.remove(key);
            }
        }
        if let Some(entry) = inner.projects.iter_mut().find(|entry| entry.key == key) {
            entry.changed = false;
            entry.trust = trust;
        }
        refresh(&mut inner, &before);
    }
}

/// Store one value the user typed for a declared name, keeping the others.
pub(crate) fn merge_secret(
    server: &ProjectServer,
    current: Credentials,
    name: &str,
    value: String,
) -> Result<Credentials, IpcError> {
    if !server.declares(name) {
        return Err(super::failure(ClientError::Invalid));
    }
    let mut next = server.restrict(current);
    if server.env.iter().any(|item| item == name) {
        next.env.insert(name.to_owned(), value);
    } else {
        next.headers.insert(name.to_owned(), value);
    }
    next.validate().map_err(super::failure)?;
    Ok(next)
}

/// A changed runnable set invalidates in-flight snapshots, grants and prompts.
fn refresh(inner: &mut Inner, before: &[openrows_mcp::client::ServerConfig]) {
    let after = inner.servers();
    if after.as_slice() != before {
        inner.revision = inner.revision.wrapping_add(1);
        inner.consent.retain(&after);
        inner.pending.clear();
    }
}

#[cfg(test)]
#[path = "project_state_tests.rs"]
mod tests;
