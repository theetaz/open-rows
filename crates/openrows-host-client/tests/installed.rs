// SPDX-License-Identifier: Apache-2.0
//! Installed resolver failures are typed and independent of a GUI or `WebKit`.
use openrows_driver_api::DriverError;
use openrows_host_client::{
    CircuitBreaker, DriverHostRuntime, HOST_BINARY_NAME, HostDriverId, HostFailure, LaunchOptions,
    MemoryBudget, launch_command, resolve_installed,
};
use std::{fs, path::Path};

fn layout() -> Result<(tempfile::TempDir, std::path::PathBuf), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let app = directory.path().join("openrows-app");
    Ok((directory, app))
}
fn failure(app: &Path, expected: HostFailure) {
    assert_eq!(resolve_installed(app), Err(expected));
    let error = DriverError::Host(expected);
    assert!(!error.is_connection_lost());
    assert_eq!(error.to_string(), expected.to_string());
    assert!(!error.to_string().is_empty());
}
#[test]
fn missing_helper_and_directory_are_spawn_failures() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, app) = layout()?;
    failure(&app, HostFailure::Spawn);
    fs::create_dir(directory.path().join(HOST_BINARY_NAME))?;
    failure(&app, HostFailure::Spawn);
    Ok(())
}
#[test]
fn installed_helper_is_beside_app_without_search_paths() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, app) = layout()?;
    let helper = directory.path().join(HOST_BINARY_NAME);
    fs::copy(std::env::current_exe()?, &helper)?;
    assert_eq!(resolve_installed(&app)?, helper);
    assert_eq!(
        resolve_installed(Path::new("openrows-app")),
        Err(HostFailure::Spawn)
    );
    // A plausible candidate in another directory cannot be a fallback.
    let missing = directory.path().join("elsewhere/openrows-app");
    failure(&missing, HostFailure::Spawn);
    Ok(())
}
#[cfg(unix)]
#[test]
fn non_executable_helper_is_spawn_failure() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    let (directory, app) = layout()?;
    let helper = directory.path().join(HOST_BINARY_NAME);
    fs::copy(std::env::current_exe()?, &helper)?;
    fs::set_permissions(helper, fs::Permissions::from_mode(0o600))?;
    failure(&app, HostFailure::Spawn);
    Ok(())
}
#[test]
fn script_and_wrong_architecture_are_incompatible() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, app) = layout()?;
    let helper = directory.path().join(HOST_BINARY_NAME);
    for contents in [
        b"#!/bin/sh\nprintf garbage\nexit 0\n".to_vec(),
        foreign_header(),
    ] {
        fs::write(&helper, contents)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&helper, fs::Permissions::from_mode(0o700))?;
        }
        failure(&app, HostFailure::Incompatible);
    }
    Ok(())
}
fn foreign_header() -> Vec<u8> {
    let mut header = vec![0; 128];
    if cfg!(target_os = "macos") {
        header[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
        let cpu = if cfg!(target_arch = "aarch64") {
            0x0100_0007_u32
        } else {
            0x0100_000c
        };
        header[4..8].copy_from_slice(&cpu.to_le_bytes());
        header[12] = 2;
    } else if cfg!(windows) {
        header[..2].copy_from_slice(b"MZ");
        header[60] = 64;
        header[64..68].copy_from_slice(b"PE\0\0");
        let cpu = if cfg!(target_arch = "aarch64") {
            0x8664_u16
        } else {
            0xaa64
        };
        header[68..70].copy_from_slice(&cpu.to_le_bytes());
    } else {
        header[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        let cpu = if cfg!(target_arch = "aarch64") {
            62_u16
        } else {
            183
        };
        header[18..20].copy_from_slice(&cpu.to_le_bytes());
    }
    header
}
#[test]
fn runtime_preserves_host_failure_and_does_not_open_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, app) = layout()?;
    let runtime = DriverHostRuntime::new(app, directory.path().join("endpoints"));
    assert!(matches!(
        runtime.launcher(HostDriverId::Probe, "test-only"),
        Err(DriverError::Host(HostFailure::Spawn))
    ));
    assert!(!directory.path().join("endpoints").exists());
    Ok(())
}
#[test]
fn typed_driver_argument_is_the_entire_secret_free_command_surface()
-> Result<(), Box<dyn std::error::Error>> {
    for driver in [
        HostDriverId::Probe,
        HostDriverId::SqliteTest,
        HostDriverId::Duckdb,
    ] {
        let command = launch_command(&LaunchOptions {
            executable: std::env::current_exe()?,
            driver,
            endpoint_root: std::env::temp_dir().join("test-only"),
            memory: MemoryBudget::default(),
            circuit: CircuitBreaker::default(),
        })?;
        let arguments: Vec<_> = command.as_std().get_args().collect();
        assert_eq!(arguments, [driver.argument()]);
        assert!(!format!("{command:?}").contains("test-only"));
        assert!(
            command
                .as_std()
                .get_envs()
                .all(|(key, _)| key == "LANG" || cfg!(windows) && key == "SystemRoot")
        );
    }
    assert_eq!(HostDriverId::parse("sqlite; test-only"), None);
    Ok(())
}
#[cfg(debug_assertions)]
#[test]
fn debug_override_requires_explicit_absolute_path() -> Result<(), Box<dyn std::error::Error>> {
    let (directory, app) = layout()?;
    let runtime = DriverHostRuntime::new(app.clone(), directory.path().join("endpoints"));
    assert!(matches!(
        runtime.with_debug_helper("relative".into()),
        Err(HostFailure::Spawn)
    ));
    let runtime = DriverHostRuntime::new(app, directory.path().join("endpoints"))
        .with_debug_helper(std::env::current_exe()?)?;
    runtime.launcher(HostDriverId::Probe, "test-only")?;
    Ok(())
}

#[tokio::test]
async fn unrelated_native_binary_exits_as_incompatible() -> Result<(), Box<dyn std::error::Error>> {
    let root = if cfg!(unix) {
        "/tmp".into()
    } else {
        std::env::temp_dir()
    };
    let directory = tempfile::Builder::new().prefix(".e-").tempdir_in(root)?;
    let options = LaunchOptions {
        executable: std::env::current_exe()?,
        driver: HostDriverId::Probe,
        endpoint_root: directory.path().to_path_buf(),
        memory: MemoryBudget::default(),
        circuit: CircuitBreaker::default(),
    };
    let result = openrows_host_client::HostSupervisor::new(options)
        .spawn()
        .await;
    assert!(
        matches!(result, Err(HostFailure::Incompatible)),
        "unrelated helper outcome: {:?}",
        result.as_ref().err()
    );
    assert_eq!(fs::read_dir(directory.path())?.count(), 0);
    Ok(())
}
