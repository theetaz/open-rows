// SPDX-License-Identifier: Apache-2.0
//! Dedicated metadata sessions (docs/10 §1): one connection per open
//! connection and database, opened lazily, so catalog reads never queue
//! behind user queries. `PostgreSQL` cannot switch databases on a connection,
//! so each browsed database gets its own.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use openrows_driver_api::{BoxFuture, DriverError, Session};
use openrows_driver_postgres::PgSession;

use super::error::{IpcError, IpcErrorKind};
use super::pg::{self, Target};

/// Saved connection id and database name.
type Key = (String, String);
type Slot = Arc<tokio::sync::Mutex<Option<PgSession>>>;

/// Metadata sessions by connection and database.
#[derive(Default)]
pub(crate) struct MetadataSessions {
    slots: Mutex<HashMap<Key, Slot>>,
}

impl MetadataSessions {
    /// Run `read` on the metadata session for `profile` and `database`,
    /// connecting to `target()` (switched to `database`) first when there is
    /// none, so the keychain is read only on connect. Calls for the same key
    /// run one at a time; a broken connection is dropped so the next call
    /// reconnects.
    ///
    /// # Errors
    /// The target, connect or `read` failure.
    pub(crate) async fn read<T>(
        &self,
        (profile, database): (&str, &str),
        target: impl FnOnce() -> Result<Target, IpcError>,
        read: impl for<'s> FnOnce(&'s mut PgSession) -> BoxFuture<'s, Result<T, DriverError>>,
    ) -> Result<T, IpcError> {
        let slot = self.slot((profile.to_owned(), database.to_owned()));
        let mut guard = slot.lock().await;
        if guard.is_none() {
            let target = target()?
                .with_database(database)
                .with_application_suffix("metadata");
            let connected = pg::connect(&target)
                .await
                .map_err(|error| IpcError::from(&error))?;
            *guard = Some(connected.session);
        }
        let Some(session) = guard.as_mut() else {
            return Err(IpcError::new(
                IpcErrorKind::Internal,
                "The metadata session was not stored",
            ));
        };
        let result = read(session).await;
        if result.as_ref().is_err_and(breaks_session) {
            *guard = None;
        }
        result.map_err(|error| IpcError::from(&error))
    }

    /// Disconnect every metadata session of `profile`.
    pub(crate) async fn close(&self, profile: &str) {
        let slots: Vec<Slot> = {
            let mut map = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
            let keys: Vec<Key> = map.keys().filter(|key| key.0 == profile).cloned().collect();
            keys.iter().filter_map(|key| map.remove(key)).collect()
        };
        for slot in slots {
            if let Some(session) = slot.lock().await.take() {
                // Closing is a courtesy goodbye; the socket is dropped either way.
                let _ = Box::new(session).close().await;
            }
        }
    }

    fn slot(&self, key: Key) -> Slot {
        let mut map = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        map.entry(key).or_default().clone()
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
}
