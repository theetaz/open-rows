// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;

use openrows_secrets::MemorySecretStore;
use openrows_store::profile::{EnvKind, EnvTag, ProfileFiles};

use super::super::error::IpcErrorKind;
use super::super::profiles::tests::create_passwordless;
use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn service(dir: &tempfile::TempDir) -> ProfileService {
    ProfileService::new(
        ProfileFiles::new(&dir.path().join("connections")),
        Arc::new(MemorySecretStore::new()),
    )
}

/// A saved `SQLite` profile on `file`; returns its id.
fn sqlite_profile(service: &ProfileService, file: FileParams) -> Result<ProfileId, IpcError> {
    let profile = ConnectionProfile::new_file("Local", file, EnvTag::new(EnvKind::Local));
    Ok(service.create(profile, None)?.id)
}

fn database_file(dir: &tempfile::TempDir, name: &str) -> Result<String, std::io::Error> {
    let path = dir.path().join(name);
    std::fs::write(&path, b"")?;
    Ok(path.display().to_string())
}

fn message(result: Result<(ConnectionProfile, BoundSql), IpcError>) -> String {
    result.err().map(|error| error.message).unwrap_or_default()
}

#[test]
fn attach_saves_the_file_and_returns_the_statement() -> TestResult {
    let dir = tempfile::tempdir()?;
    let service = service(&dir);
    let read_only = FileParams {
        path: dir.path().join("main.db"),
        read_only: true,
        ..FileParams::default()
    };
    let id = sqlite_profile(&service, read_only)?;
    let logs = database_file(&dir, "logs.db")?;
    let (saved, statement) = attach_saved(&service, id, &format!(" {logs} "), " logs ")?;
    assert_eq!(statement.sql, "ATTACH DATABASE ?1 AS \"logs\"");
    let uri = statement
        .params
        .first()
        .cloned()
        .flatten()
        .unwrap_or_default();
    assert!(uri.contains(&format!("{logs}?mode=ro&vfs=")), "{uri}");
    let stored = service.get(id)?.file.unwrap_or_default().attached;
    assert_eq!(stored, saved.file.unwrap_or_default().attached);
    assert_eq!(stored.first().map(|file| file.alias.as_str()), Some("logs"));
    Ok(())
}

#[test]
fn attach_refuses_bad_aliases_missing_files_and_other_engines() -> TestResult {
    let dir = tempfile::tempdir()?;
    let service = service(&dir);
    let id = sqlite_profile(&service, FileParams::memory())?;
    let logs = database_file(&dir, "logs.db")?;
    attach_saved(&service, id, &logs, "logs")?;
    let cases = [
        (
            logs.as_str(),
            "LOGS",
            "Schema name is already used by another attached database",
        ),
        (logs.as_str(), "main", "Schema name cannot be main or temp"),
        (logs.as_str(), " ", "Schema name must not be empty"),
        (
            "/no/such/file.db",
            "other",
            "Choose an existing database file to attach",
        ),
        (
            "relative.db",
            "other",
            "Attached database file must be an absolute path",
        ),
    ];
    for (path, alias, expected) in cases {
        assert_eq!(message(attach_saved(&service, id, path, alias)), expected);
    }
    assert_eq!(service.get(id)?.file.unwrap_or_default().attached.len(), 1);
    let postgres = profile_id(&create_passwordless(&service)?)?;
    let refused = attach_saved(&service, postgres, &logs, "logs").err();
    assert!(refused.is_some_and(|error| error.kind == IpcErrorKind::Invalid));
    Ok(())
}

#[test]
fn detach_removes_the_saved_file_by_alias() -> TestResult {
    let dir = tempfile::tempdir()?;
    let service = service(&dir);
    let id = sqlite_profile(&service, FileParams::memory())?;
    let logs = database_file(&dir, "logs.db")?;
    attach_saved(&service, id, &logs, "Logs")?;
    let (saved, statement) = detach_saved(&service, id, "logs")?;
    assert_eq!(statement.sql, "DETACH DATABASE \"Logs\"");
    assert!(saved.file.unwrap_or_default().attached.is_empty());
    assert_eq!(
        message(detach_saved(&service, id, "logs")),
        "Schema name matches no attached database"
    );
    Ok(())
}

#[test]
fn warnings_name_the_reason() {
    let warning = session_warning(Some("cannot ATTACH database within transaction"));
    assert!(warning.contains("within transaction") && warning.contains("reopened"));
    assert!(session_warning(None).contains("cancelled"));
}
