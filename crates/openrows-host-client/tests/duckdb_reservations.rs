// SPDX-License-Identifier: Apache-2.0
//! Parent-only file identity and lease lifetime, without a native engine dependency.
use openrows_driver_api::{DriverError, FileSource};
use openrows_host_client::FileReservations;
use std::path::Path;

fn file(path: &Path, read_only: bool) -> FileSource {
    FileSource::Disk {
        path: path.into(),
        read_only,
        create: false,
    }
}
#[test]
fn writer_refuses_aliases_and_readers_until_released() -> Result<(), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir_in(".")?;
    let path = root.path().join("data.duckdb");
    let registry = FileReservations::default();
    let writer = registry.reserve(&file(&path, false))?;
    std::fs::write(&path, b"test-only")?;
    let absolute = std::fs::canonicalize(&path)?;
    let hard = root.path().join("hard.duckdb");
    std::fs::hard_link(&path, &hard)?;
    for alias in [&path, &absolute, &hard] {
        for read_only in [false, true] {
            assert!(
                matches!(registry.reserve(&file(alias, read_only)), Err(DriverError::Server { code, .. }) if code == "55006")
            );
        }
    }
    #[cfg(unix)]
    {
        let alias = root.path().join("symbolic.duckdb");
        std::os::unix::fs::symlink(&absolute, &alias)?;
        assert!(registry.reserve(&file(&alias, false)).is_err());
    }
    drop(writer);
    let reader = registry.reserve(&file(&absolute, true))?;
    let other = registry.reserve(&file(&hard, true))?;
    assert!(registry.reserve(&file(&path, false)).is_err());
    drop((reader, other));
    registry.reserve(&file(&path, false))?;
    Ok(())
}
#[test]
fn memory_profiles_are_exclusive_and_independent() -> Result<(), DriverError> {
    let registry = FileReservations::default();
    let source = FileSource::Memory {
        name: "test-only".into(),
    };
    let held = registry.reserve(&source)?;
    assert!(registry.reserve(&source).is_err());
    registry.reserve(&FileSource::Memory {
        name: "other-test".into(),
    })?;
    drop(held);
    registry.reserve(&source)?;
    Ok(())
}
