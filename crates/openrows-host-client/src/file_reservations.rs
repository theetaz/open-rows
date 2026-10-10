// SPDX-License-Identifier: Apache-2.0
//! Parent-only reservations; the engine's file lock remains authoritative.
use openrows_driver_api::{DriverError, FileSource};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

/// Registry shared by every hosted `DuckDB` connection in one parent process.
#[derive(Clone, Default)]
pub struct FileReservations(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    next: u64,
    held: BTreeMap<u64, Entry>,
}
struct Entry {
    identity: Identity,
    read_only: bool,
}

enum Identity {
    Memory(String),
    File {
        path: PathBuf,
        handle: Option<same_file::Handle>,
    },
}

/// RAII lease held until the owning host is closed or reaped.
pub struct FileReservation {
    registry: FileReservations,
    id: u64,
    host: Option<tokio::sync::watch::Receiver<crate::HostState>>,
}

impl FileReservations {
    /// Reserve a canonical file or profile-local memory identity before spawning.
    /// # Errors
    /// `55006` for competing owners, `58030` for unusable file paths.
    pub fn reserve(&self, source: &FileSource) -> Result<FileReservation, DriverError> {
        let identity = identity(source)?;
        let read_only = source.read_only();
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if state
            .held
            .values()
            .any(|entry| same(&entry.identity, &identity) && !(entry.read_only && read_only))
        {
            return Err(DriverError::Server {
                code: "55006".into(),
                message: "DuckDB already owns this file or memory profile. Use its shared session, or close it before opening another connection; readers cannot open while a writer is active.".into(),
            });
        }
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| path_error("reservation identity exhausted"))?;
        let id = state.next;
        state.held.insert(
            id,
            Entry {
                identity,
                read_only,
            },
        );
        Ok(FileReservation {
            registry: self.clone(),
            id,
            host: None,
        })
    }
}
impl FileReservation {
    pub(crate) fn bind(&mut self, host: &crate::HostSession) {
        let state = host.subscribe();
        let registry = self.registry.clone();
        let id = self.id;
        self.host = Some(state.clone());
        tokio::spawn(release_when_reaped(state, registry, id));
    }
    pub(crate) fn canonical_path(&self) -> Option<PathBuf> {
        let state = self
            .registry
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match &state.held.get(&self.id)?.identity {
            Identity::File { path, .. } => Some(path.clone()),
            Identity::Memory(_) => None,
        }
    }
    pub(crate) fn refresh(&self) {
        let mut state = self
            .registry
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(Entry {
            identity: Identity::File { path, handle },
            ..
        }) = state.held.get_mut(&self.id)
            && handle.is_none()
        {
            *handle = same_file::Handle::from_path(path).ok();
        }
    }
}
impl Drop for FileReservation {
    fn drop(&mut self) {
        if self
            .host
            .as_ref()
            .is_some_and(|state| *state.borrow() == crate::HostState::Ready)
        {
            return;
        }
        self.registry
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .held
            .remove(&self.id);
    }
}

fn identity(source: &FileSource) -> Result<Identity, DriverError> {
    match source {
        FileSource::Memory { name } => Ok(Identity::Memory(name.clone())),
        FileSource::Disk { path, .. } => {
            let path = canonical(path, 0)?;
            let handle = same_file::Handle::from_path(&path).ok();
            Ok(Identity::File { path, handle })
        }
    }
}
fn canonical(path: &Path, depth: u8) -> Result<PathBuf, DriverError> {
    if depth > 32 {
        return Err(path_error("too many symbolic links"));
    }
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Ok(target) = std::fs::read_link(path) {
                let parent = path.parent().unwrap_or_else(|| Path::new("."));
                return canonical(&parent.join(target), depth + 1);
            }
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let parent = std::fs::canonicalize(parent).map_err(|e| path_error(&e.to_string()))?;
            let name = path
                .file_name()
                .ok_or_else(|| path_error("choose a database file"))?;
            Ok(parent.join(name))
        }
        Err(error) => Err(path_error(&error.to_string())),
    }
}
fn same(left: &Identity, right: &Identity) -> bool {
    match (left, right) {
        (Identity::Memory(left), Identity::Memory(right)) => left == right,
        (
            Identity::File {
                path: left,
                handle: lhs,
            },
            Identity::File {
                path: right,
                handle: rhs,
            },
        ) => {
            left == right
                || lhs.as_ref().zip(rhs.as_ref()).is_some_and(|(a, b)| a == b)
                || same_file::is_same_file(left, right).unwrap_or(false)
        }
        (Identity::Memory(_), Identity::File { .. })
        | (Identity::File { .. }, Identity::Memory(_)) => false,
    }
}
fn path_error(message: &str) -> DriverError {
    DriverError::Server {
        code: "58030".into(),
        message: format!("DuckDB file reservation: {message}"),
    }
}

pub(crate) fn process_registry() -> FileReservations {
    static REGISTRY: std::sync::OnceLock<FileReservations> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(FileReservations::default).clone()
}

async fn release_when_reaped(
    mut state: tokio::sync::watch::Receiver<crate::HostState>,
    registry: FileReservations,
    id: u64,
) {
    while *state.borrow_and_update() == crate::HostState::Ready {
        // A missing lifecycle notification is not proof that a file lock is gone.
        if state.changed().await.is_err() {
            return;
        }
    }
    registry
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .held
        .remove(&id);
}
