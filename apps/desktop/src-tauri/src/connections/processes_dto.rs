// SPDX-License-Identifier: Apache-2.0
//! Webview mirrors of the driver process list (FR-ADMIN-01).
use openrows_driver_api::{ProcessInfo, ProcessSignal};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::error::{IpcError, IpcErrorKind};

/// One server process as the Processes view lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcessDto {
    /// Backend pid or connection id (32-bit, like the `MySQL` handshake's).
    id: u32,
    user: Option<String>,
    database: Option<String>,
    /// Client address and port; `None` for local sockets.
    client: Option<String>,
    /// Application name the client reported.
    application: Option<String>,
    /// e.g. `active`, `idle in transaction`, `Query`, `Sleep`.
    state: Option<String>,
    /// What the process waits for, e.g. `Lock: relation` or `User sleep`.
    wait: Option<String>,
    /// When the current or last statement started, as server text.
    query_start: Option<String>,
    /// How long the current state or statement has run.
    duration_ms: Option<f64>,
    /// The current or last statement.
    query: Option<String>,
    /// The connection the list runs on (the app's metadata connection).
    current: bool,
    /// Reports this app's application name: one of the app's own connections.
    app: bool,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "durations stay far below 2^53 ms"
)]
fn milliseconds(value: u64) -> f64 {
    value as f64
}

impl ProcessDto {
    /// `process` as listed, `app` when its application name starts with `application`.
    ///
    /// # Errors
    /// `Protocol` for an id beyond 32 bits.
    pub(crate) fn new(process: ProcessInfo, application: &str) -> Result<Self, IpcError> {
        let id = u32::try_from(process.id).map_err(|_| {
            IpcError::new(
                IpcErrorKind::Protocol,
                format!("process id {} is beyond 32 bits", process.id),
            )
        })?;
        let app = process
            .application
            .as_deref()
            .is_some_and(|name| name.starts_with(application));
        Ok(Self {
            id,
            user: process.user,
            database: process.database,
            client: process.client,
            application: process.application,
            state: process.state,
            wait: process.wait,
            query_start: process.query_start,
            duration_ms: process.duration_ms.map(milliseconds),
            query: process.query,
            current: process.current,
            app,
        })
    }
}

/// What to do to a process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProcessSignalDto {
    /// Abort the running statement; the session stays connected.
    CancelQuery,
    /// End the session; its open transaction rolls back.
    Terminate,
}

impl From<ProcessSignalDto> for ProcessSignal {
    fn from(signal: ProcessSignalDto) -> Self {
        match signal {
            ProcessSignalDto::CancelQuery => Self::CancelQuery,
            ProcessSignalDto::Terminate => Self::Terminate,
        }
    }
}

/// Cancel or terminate one process, after the user confirmed it.
#[derive(Clone, Debug, PartialEq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProcessSignalRequest {
    pub(crate) profile_id: String,
    /// The database whose metadata connection sends the signal.
    pub(crate) database: String,
    pub(crate) process_id: u32,
    pub(crate) signal: ProcessSignalDto,
    /// The user confirmed signalling the connection the list runs on.
    pub(crate) allow_own: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(process_id: u32) -> ProcessSignalRequest {
        ProcessSignalRequest {
            profile_id: "p".to_owned(),
            database: "db".to_owned(),
            process_id,
            signal: ProcessSignalDto::Terminate,
            allow_own: false,
        }
    }

    #[test]
    fn own_connections_are_marked_by_application_name() -> Result<(), IpcError> {
        let listed = |application: Option<&str>| ProcessInfo {
            id: 7,
            application: application.map(str::to_owned),
            duration_ms: Some(250),
            ..ProcessInfo::default()
        };
        let own = ProcessDto::new(listed(Some("OpenRows metadata")), "OpenRows")?;
        assert!(own.app);
        assert_eq!((own.id, own.duration_ms), (7, Some(250.0)));
        assert!(!ProcessDto::new(listed(Some("psql")), "OpenRows")?.app);
        assert!(!ProcessDto::new(listed(None), "OpenRows")?.app);
        let wide = ProcessInfo {
            id: 1 << 40,
            ..ProcessInfo::default()
        };
        assert!(ProcessDto::new(wide, "OpenRows").is_err());
        Ok(())
    }

    #[test]
    fn signals_map_to_the_driver() {
        assert_eq!(
            ProcessSignal::from(ProcessSignalDto::CancelQuery),
            ProcessSignal::CancelQuery
        );
        assert_eq!(
            ProcessSignal::from(request(1).signal),
            ProcessSignal::Terminate
        );
    }
}
