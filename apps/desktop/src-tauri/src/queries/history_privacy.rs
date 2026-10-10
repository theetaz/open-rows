// SPDX-License-Identifier: Apache-2.0
//! History policy and the literal-review IPC shared by query and grid writes.
use crate::connections::{EngineDto, IpcError, drivers::driver};
use openrows_sql::{Dialect, literal_ranges, redact_literals};
use openrows_store::profile::{ConnectionProfile, EnvKind};
use openrows_store::{HistoryRecord, LocalStore};
use serde::Serialize;
use specta::Type;
/// Refuse redacted SQL before resolving a session or executing anything.
pub(crate) fn validate_reuse(sql: &str) -> Result<(), IpcError> {
    if openrows_sql::has_redacted_literals(sql) {
        return Err(IpcError::invalid(
            "Fill every history redaction placeholder before running SQL",
        ));
    }
    Ok(())
}
/// Whether this connection removes literals from persisted history.
pub(crate) fn redacts(profile: &ConnectionProfile) -> bool {
    profile.redact_history || profile.env_tag.kind == EnvKind::Prod || profile.safety.read_only
}
/// Apply privacy before anything reaches the local database or its FTS index.
pub(crate) fn record(local: &LocalStore, mut record: HistoryRecord, privacy: (bool, Dialect)) {
    if privacy.0 {
        record.statement = redact_literals(&record.statement, privacy.1);
        // Server messages can echo SQL and parameter values; do not persist them.
        record.error_message = None;
    }
    let _ = local.record_history(&record);
}
/// UTF-16 positions of one SQL literal for the webview's review highlights.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiteralRangeDto {
    from: u32,
    to: u32,
}
/// Inspect literals locally without opening a session or executing SQL.
#[tauri::command]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "Tauri owns command input")]
pub(crate) fn history_literals(sql: String, engine: EngineDto) -> Vec<LiteralRangeDto> {
    let mut byte_offset = 0;
    let mut utf16_offset = 0;
    literal_ranges(&sql, driver(engine.into()).dialect())
        .into_iter()
        .filter_map(|range| {
            utf16_offset += sql.get(byte_offset..range.start)?.encode_utf16().count();
            let from = u32::try_from(utf16_offset).ok()?;
            utf16_offset += sql.get(range.clone())?.encode_utf16().count();
            byte_offset = range.end;
            Some(LiteralRangeDto {
                from,
                to: u32::try_from(utf16_offset).ok()?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use openrows_store::profile::{Driver, EnvTag};
    use openrows_store::{HistorySource, HistoryStatus};
    #[test]
    fn unfilled_history_markers_cannot_reach_execution_or_explain() {
        for sql in [
            "SELECT __OPENROWS_REDACTED_1__",
            "SELECT __openrows_redacted_2__",
            "SELECT '__OPENROWS_REDACTED_1__'",
        ] {
            assert!(validate_reuse(sql).is_err());
        }
        assert!(validate_reuse("SELECT 'x'").is_ok());
    }
    #[test]
    fn production_read_only_and_explicit_profiles_redact() {
        let mut profile = ConnectionProfile::new(
            "local",
            Driver::Postgres,
            openrows_store::profile::ServerParams::default(),
            EnvTag::new(EnvKind::Local),
        );
        assert!(!redacts(&profile));
        profile.redact_history = true;
        assert!(redacts(&profile));
        profile.redact_history = false;
        profile.safety.read_only = true;
        assert!(redacts(&profile));
        profile.safety.read_only = false;
        profile.env_tag.kind = EnvKind::Prod;
        assert!(redacts(&profile));
        profile.driver = Driver::Postgres;
        assert!(redacts(&profile));
    }
    #[test]
    fn stored_history_and_fts_never_contain_literals_or_server_echoes()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let local = LocalStore::open(&dir.path().join("history.sqlite"))?;
        record(
            &local,
            HistoryRecord {
                profile_id: "p".into(),
                database: None,
                started_at_ms: 1,
                duration_ms: None,
                status: HistoryStatus::Error,
                rows: None,
                statement: "SELECT 'x', 123, $$x$$, $tag$x$tag$".into(),
                error_message: Some("x".into()),
                source: HistorySource::Editor,
            },
            (true, Dialect::Postgres),
        );
        let rows = local.search_history(None, "", 10)?;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0]
                .record
                .statement
                .matches(openrows_sql::REDACTED_PREFIX)
                .count(),
            4
        );
        assert!(rows[0].record.error_message.is_none());
        assert!(local.search_history(None, "x", 10)?.is_empty());
        assert!(local.search_history(None, "123", 10)?.is_empty());
        Ok(())
    }
    #[test]
    fn review_ranges_use_utf16_for_unicode_and_dollar_strings() {
        let sql = "SELECT '😀', 42, $$x$$";
        let ranges = history_literals(sql.into(), EngineDto::Postgres);
        assert_eq!(ranges.len(), 3);
        assert_eq!((ranges[0].from, ranges[0].to), (7, 11));
    }

    #[test]
    fn libsql_flavours_review_and_redact_using_sqlite_literals() {
        let sql = "SELECT 'test-only', X'00ff', 123, :value, \"identifier\"";
        for engine in [EngineDto::Libsql, EngineDto::LibsqlFile] {
            let dialect = driver(engine.into()).dialect();
            assert_eq!(dialect, Dialect::Sqlite);
            assert_eq!(history_literals(sql.into(), engine).len(), 3);
            let redacted = redact_literals(sql, dialect);
            assert!(!redacted.contains("test-only"));
            assert!(!redacted.contains("00ff"));
            assert!(!redacted.contains("123"));
            assert!(redacted.contains(":value"));
            assert!(redacted.contains("\"identifier\""));
            assert!(validate_reuse(&redacted).is_err());
        }
    }
}
