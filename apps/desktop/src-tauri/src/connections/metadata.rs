// SPDX-License-Identifier: Apache-2.0
//! Dedicated metadata sessions (docs/10 §1): one connection per open
//! connection and database, opened lazily, so catalog reads never queue
//! behind user queries. `PostgreSQL` cannot switch databases on a connection,
//! so each browsed database gets its own (for every engine, to keep one model).
//! A connection idle for a while is pinged before use and replaced when it
//! died; after wake or a network change idle ones are dropped (M1-08).
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use openrows_driver_api::{BoxFuture, Connection, DriverError};

use super::error::{IpcError, IpcErrorKind};
use super::target::{self, Target};

/// Idle time after which a metadata connection is pinged before use.
const STALE_AFTER: Duration = Duration::from_secs(10);
/// How long that ping may take before the connection is replaced.
const PING_TIMEOUT: Duration = Duration::from_secs(5);

/// Saved connection id and database name.
type Key = (String, String);
pub(super) type Slot = Arc<tokio::sync::Mutex<Option<Held>>>;

/// An open metadata connection and when it last answered.
pub(super) struct Held {
    connection: Box<dyn Connection>,
    used: Instant,
    pub(super) lifecycle: super::metadata_lifecycle::Observer,
}

/// Metadata sessions by connection and database.
#[derive(Default)]
pub(crate) struct MetadataSessions {
    slots: Mutex<HashMap<Key, Slot>>,
}

impl MetadataSessions {
    /// Run `read` on the metadata session for `profile` and `database`,
    /// connecting to `target()` (switched to `database`) first when there is
    /// none, so the keychain is read (and an SSH tunnel opened) only on connect. Calls for the same key
    /// run one at a time; a broken connection is dropped so the next call
    /// reconnects.
    ///
    /// # Errors
    /// The target, connect or `read` failure.
    pub(crate) async fn read<T, F>(
        &self,
        (profile, database): (&str, &str),
        target: impl FnOnce() -> F,
        read: impl for<'s> FnOnce(&'s mut dyn Connection) -> BoxFuture<'s, Result<T, DriverError>>,
    ) -> Result<T, IpcError>
    where
        F: Future<Output = Result<Target, IpcError>>,
    {
        let slot = self.slot((profile.to_owned(), database.to_owned()));
        let mut guard = slot.lock().await;
        drop_if_dead(&mut guard).await;
        if guard.is_none() {
            let target = target()
                .await?
                .with_database(database)
                .with_application_suffix("metadata");
            if target.driver.dialect() == openrows_sql::Dialect::Duckdb {
                return Err(IpcError::new(
                    IpcErrorKind::Unsupported,
                    "Connect the DuckDB connection first; its catalog is read on that one session and no extra connection is opened",
                ));
            }
            let connected = target::connect(&target)
                .await
                .map_err(|error| IpcError::from(&error))?;
            let lifecycle = super::metadata_lifecycle::Observer::new(
                connected.connection.as_ref(),
                Arc::downgrade(&slot),
            );
            *guard = Some(Held {
                connection: connected.connection,
                used: Instant::now(),
                lifecycle,
            });
        }
        let mut lease = ReadLease::new(&mut guard)?;
        let result = read(lease.connection()?).await;
        lease.complete(!result.as_ref().is_err_and(breaks_session));
        result.map_err(|error| IpcError::from(&error))
    }

    /// Forget every idle metadata connection (after wake or a network
    /// change they may be half-open); the next read reconnects. Busy ones
    /// are left to finish.
    pub(crate) fn reset_idle(&self) {
        let slots: Vec<Slot> = self
            .slots
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        for slot in slots {
            if let Ok(mut guard) = slot.try_lock() {
                // No goodbye: it could wait on a dead link; dropping closes the socket.
                guard.take();
            }
        }
    }

    /// Saved connections that have metadata sessions.
    pub(crate) fn profiles(&self) -> Vec<String> {
        let map = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        let mut profiles: Vec<String> = map.keys().map(|key| key.0.clone()).collect();
        profiles.sort_unstable();
        profiles.dedup();
        profiles
    }

    /// Close the metadata connections of `profile` unused for `idle` (the
    /// idle timeout); busy ones are left, the next read reconnects.
    pub(crate) async fn close_idle(&self, profile: &str, idle: Duration) {
        let slots: Vec<Slot> = {
            let map = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
            map.iter()
                .filter(|(key, _)| key.0 == profile)
                .map(|(_, slot)| slot.clone())
                .collect()
        };
        for slot in slots {
            let Ok(mut guard) = slot.try_lock() else {
                continue;
            };
            if let Some(held) = guard.take_if(|held| held.used.elapsed() >= idle) {
                // Closing is a courtesy goodbye; the socket is dropped either way.
                let _ = held.connection.close().await;
            }
        }
    }

    /// Disconnect every metadata session of `profile`.
    pub(crate) async fn close(&self, profile: &str) {
        self.close_where(|key| key.0 == profile).await;
    }

    /// Disconnect the metadata session of `profile` on `database`.
    pub(crate) async fn close_database(&self, profile: &str, database: &str) {
        self.close_where(|key| key.0 == profile && key.1 == database)
            .await;
    }

    async fn close_where(&self, matches: impl Fn(&Key) -> bool) {
        let slots: Vec<Slot> = {
            let mut map = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
            let keys: Vec<Key> = map.keys().filter(|key| matches(key)).cloned().collect();
            keys.iter().filter_map(|key| map.remove(key)).collect()
        };
        for slot in slots {
            if let Some(held) = slot.lock().await.take() {
                // Closing is a courtesy goodbye; the socket is dropped either way.
                let _ = held.connection.close().await;
            }
        }
    }

    fn slot(&self, key: Key) -> Slot {
        let mut map = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        map.entry(key).or_default().clone()
    }
}

/// A dropped read discards its connection instead of leaving a busy wire in the pool.
struct ReadLease<'a> {
    slot: &'a mut Option<Held>,
    held: Option<Held>,
    cancel: Option<Arc<dyn openrows_driver_api::CancelHandle>>,
}
impl<'a> ReadLease<'a> {
    fn new(slot: &'a mut Option<Held>) -> Result<Self, IpcError> {
        let held = slot.take().ok_or_else(missing_session)?;
        let cancel = held.connection.cancel_handle();
        Ok(Self {
            slot,
            held: Some(held),
            cancel,
        })
    }
    fn connection(&mut self) -> Result<&mut dyn Connection, IpcError> {
        match self.held.as_mut() {
            Some(held) => Ok(held.connection.as_mut()),
            None => Err(missing_session()),
        }
    }
    fn complete(mut self, keep: bool) {
        self.cancel = None;
        if keep {
            if let Some(held) = self.held.as_mut() {
                held.used = Instant::now();
            }
            *self.slot = self.held.take();
        }
    }
}
impl Drop for ReadLease<'_> {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            tauri::async_runtime::spawn(async move {
                let _ = tokio::time::timeout(Duration::from_secs(1), cancel.cancel()).await;
            });
        }
    }
}
fn missing_session() -> IpcError {
    IpcError::new(
        IpcErrorKind::Internal,
        "The metadata session was not stored",
    )
}

/// A connection that idled for a while may have died unnoticed: ping it
/// (a read that never started is safe to run on a fresh one) and drop it
/// when it does not answer.
async fn drop_if_dead(slot: &mut Option<Held>) {
    let Some(held) = slot
        .as_mut()
        .filter(|held| held.used.elapsed() >= STALE_AFTER)
    else {
        return;
    };
    match tokio::time::timeout(PING_TIMEOUT, held.connection.ping()).await {
        Ok(Ok(()) | Err(DriverError::Server { .. })) => held.used = Instant::now(),
        Ok(Err(_)) | Err(_) => *slot = None,
    }
}

/// Server errors leave the connection usable; anything else may not.
fn breaks_session(error: &DriverError) -> bool {
    !matches!(error, DriverError::Server { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_server_errors_keep_the_session() {
        let server = DriverError::Server {
            code: "42P01".to_owned(),
            message: "missing".to_owned(),
        };
        assert!(!breaks_session(&server));
        assert!(breaks_session(&DriverError::Protocol("bad".to_owned())));
        assert!(breaks_session(&DriverError::Network(
            std::io::ErrorKind::BrokenPipe.into()
        )));
    }

    #[test]
    fn each_connection_and_database_has_its_own_slot() {
        let sessions = MetadataSessions::default();
        let first = sessions.slot(("p".to_owned(), "db".to_owned()));
        let again = sessions.slot(("p".to_owned(), "db".to_owned()));
        let other = sessions.slot(("p".to_owned(), "reports".to_owned()));
        assert!(Arc::ptr_eq(&first, &again));
        assert!(!Arc::ptr_eq(&first, &other));
    }

    #[test]
    fn closing_a_database_keeps_the_other_slots() {
        let sessions = MetadataSessions::default();
        let orders = sessions.slot(("p".to_owned(), "orders".to_owned()));
        let reports = sessions.slot(("p".to_owned(), "reports".to_owned()));
        tauri::async_runtime::block_on(sessions.close_database("p", "orders"));
        let reopened = sessions.slot(("p".to_owned(), "orders".to_owned()));
        assert!(!Arc::ptr_eq(&orders, &reopened));
        let kept = sessions.slot(("p".to_owned(), "reports".to_owned()));
        assert!(Arc::ptr_eq(&reports, &kept));
    }
}

#[cfg(test)]
#[path = "metadata_host_tests.rs"]
mod host_tests;
