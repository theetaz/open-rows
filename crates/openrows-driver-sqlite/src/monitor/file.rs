// SPDX-License-Identifier: Apache-2.0
//! Cheap file/page diagnostics: three integer PRAGMAs and `journal_mode`,
//! file metadata and the documented WAL-index header
//! (<https://www.sqlite.org/walformat.html>). No checkpoint, integrity or
//! quick check runs, so a tick never rewrites or scans the database.
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use openrows_monitor::{Column, MetricKind, Reading, SampleState, Totals};
use rusqlite::{Connection, ErrorCode};

use super::MONITOR_BUSY;
use crate::session::SqliteSession;

/// `iVersion` of every initialised WAL-index header.
const WAL_INDEX_VERSION: u32 = 3_007_000;

struct Stats {
    page_size: i64,
    page_count: i64,
    freelist: i64,
    journal: String,
    file_bytes: Option<u64>,
    wal_bytes: Option<u64>,
    wal: Option<WalIndex>,
}

/// Frames in the WAL and how many a checkpoint has already copied back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WalIndex {
    pub(super) frames: u32,
    pub(super) backfilled: u32,
}

/// One bounded Database reading; a timeout interrupts the monitor connection.
pub(super) async fn read(session: &mut SqliteSession, deadline: u32) -> Reading {
    let path = session.monitor.path.clone();
    let work = session
        .worker
        .call(move |connection| sample(connection, path.as_deref()));
    let limit = Duration::from_millis(u64::from(deadline.clamp(4, 1_000)));
    match tokio::time::timeout(limit, work).await {
        Ok(Ok(Ok(stats))) => {
            let ledger = &session.monitor.ledger;
            reading(&stats, ledger.totals(), ledger.epoch())
        }
        Ok(Ok(Err(error))) => Reading::missing(failure(&error)),
        Ok(Err(_)) => {
            let mut reading = Reading::missing(SampleState::Gap);
            reading.terminal = true;
            reading
        }
        Err(_) => {
            if let Some(cancel) = openrows_driver_api::Session::cancel_handle(session) {
                let _ = cancel.cancel().await;
            }
            Reading::missing(SampleState::Gap)
        }
    }
}

fn sample(connection: &Connection, path: Option<&Path>) -> rusqlite::Result<Stats> {
    connection.busy_timeout(MONITOR_BUSY)?;
    let int = |pragma: &str| {
        connection.query_row(&format!("PRAGMA main.{pragma}"), [], |row| {
            row.get::<_, i64>(0)
        })
    };
    let journal = connection.query_row("PRAGMA main.journal_mode", [], |row| row.get(0))?;
    let sibling = |suffix: &str| {
        path.map(|path| {
            let mut name = path.as_os_str().to_owned();
            name.push(suffix);
            PathBuf::from(name)
        })
    };
    Ok(Stats {
        page_size: int("page_size")?,
        page_count: int("page_count")?,
        freelist: int("freelist_count")?,
        journal,
        file_bytes: path.and_then(size),
        wal_bytes: sibling("-wal").as_deref().and_then(size),
        wal: sibling("-shm").as_deref().and_then(wal_index),
    })
}

fn size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|meta| meta.len())
}

fn wal_index(path: &Path) -> Option<WalIndex> {
    let mut header = [0_u8; 100];
    File::open(path).ok()?.read_exact(&mut header).ok()?;
    parse_wal_index(&header)
}

/// `mxFrame` and `nBackfill` from an initialised header whose two copies
/// agree; a header mid-update is unknown rather than guessed.
pub(super) fn parse_wal_index(header: &[u8; 100]) -> Option<WalIndex> {
    let word = |at: usize| {
        header
            .get(at..at + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_ne_bytes)
    };
    let stable = header.get(..48) == header.get(48..96);
    let initialised = word(0)? == WAL_INDEX_VERSION && header.get(12) == Some(&1);
    (stable && initialised).then_some(WalIndex {
        frames: word(16)?,
        backfilled: word(96)?,
    })
}

fn failure(error: &rusqlite::Error) -> SampleState {
    match error.sqlite_error_code() {
        Some(
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked | ErrorCode::OperationInterrupted,
        ) => SampleState::Gap,
        Some(
            ErrorCode::PermissionDenied
            | ErrorCode::CannotOpen
            | ErrorCode::AuthorizationForStatementDenied,
        ) => SampleState::PermissionDenied(
            "File permissions refuse this read; OpenRows changes no file modes".into(),
        ),
        _ => SampleState::Unavailable("File statistics could not be read from this file".into()),
    }
}

fn reading(stats: &Stats, totals: Totals, epoch: &str) -> Reading {
    let gauge = |name: &str, unit: &str| Column {
        name: name.into(),
        unit: unit.into(),
        kind: MetricKind::Gauge,
    };
    let text = |name: &str| Column {
        name: name.into(),
        unit: "text".into(),
        kind: MetricKind::Text,
    };
    let mut columns = vec![
        text("database_id"),
        gauge("page_size", "bytes"),
        gauge("page_count", "count"),
        gauge("freelist_count", "count"),
        gauge("file_bytes", "bytes"),
        gauge("wal_bytes", "bytes"),
        gauge("wal_frames", "count"),
        gauge("checkpoint_backlog", "count"),
    ];
    columns.extend(Totals::columns());
    columns.push(text("journal_mode"));
    let mut row = vec![
        Some("main".to_owned()),
        Some(stats.page_size.to_string()),
        Some(stats.page_count.to_string()),
        Some(stats.freelist.to_string()),
        stats.file_bytes.map(|v| v.to_string()),
        stats.wal_bytes.map(|v| v.to_string()),
        stats.wal.map(|wal| wal.frames.to_string()),
        stats
            .wal
            .and_then(|wal| wal.frames.checked_sub(wal.backfilled))
            .map(|v| v.to_string()),
    ];
    row.extend(totals.cells());
    row.push(Some(stats.journal.clone()));
    Reading {
        terminal: false,
        state: SampleState::Ok,
        epoch: epoch.to_owned(),
        columns,
        rows: vec![row],
        partial: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(frames: u32, backfilled: u32) -> [u8; 100] {
        let mut header = [0_u8; 100];
        let mut copy = [0_u8; 48];
        copy[..4].copy_from_slice(&WAL_INDEX_VERSION.to_ne_bytes());
        copy[12] = 1;
        copy[16..20].copy_from_slice(&frames.to_ne_bytes());
        header[..48].copy_from_slice(&copy);
        header[48..96].copy_from_slice(&copy);
        header[96..100].copy_from_slice(&backfilled.to_ne_bytes());
        header
    }

    #[test]
    fn wal_index_reads_frames_and_backfill_from_a_stable_header() {
        assert_eq!(
            parse_wal_index(&header(12, 5)),
            Some(WalIndex {
                frames: 12,
                backfilled: 5
            })
        );
    }

    #[test]
    fn torn_or_uninitialised_wal_index_is_unknown() {
        let mut torn = header(12, 5);
        torn[16] ^= 1;
        assert_eq!(parse_wal_index(&torn), None);
        assert_eq!(parse_wal_index(&[0; 100]), None);
    }
}
