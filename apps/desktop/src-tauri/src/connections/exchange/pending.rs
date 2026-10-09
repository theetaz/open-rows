// SPDX-License-Identifier: Apache-2.0
//! Single-use, bounded previews; cancellation drops zeroized password values.
use super::super::error::IpcError;
use openrows_store::profile::{ProfileId, exchange::ImportBatch};
use std::{collections::HashMap, sync::Mutex};
/// Pending source data and the profiles the user reviewed.
pub(super) struct Pending {
    pub batch: ImportBatch,
    pub snapshot: String,
}
/// Core-owned previews; no source content is persisted before confirmation.
#[derive(Default)]
pub(crate) struct PendingImports(Mutex<HashMap<String, Pending>>);
impl PendingImports {
    pub(super) fn insert(&self, pending: Pending) -> Result<String, IpcError> {
        let mut previews = self
            .0
            .lock()
            .map_err(|_| IpcError::invalid("Import previews unavailable"))?;
        if previews.len() >= 8 {
            return Err(IpcError::invalid("Close an existing import preview first"));
        }
        let token = ProfileId::new().to_string();
        previews.insert(token.clone(), pending);
        Ok(token)
    }
    pub(super) fn take(&self, token: &str) -> Result<Pending, IpcError> {
        self.0
            .lock()
            .map_err(|_| IpcError::invalid("Import previews unavailable"))?
            .remove(token)
            .ok_or_else(|| IpcError::invalid("Import preview expired; preview the file again"))
    }
    pub(super) fn discard(&self, token: &str) -> Result<(), IpcError> {
        self.take(token).map(|_| ())
    }
}
