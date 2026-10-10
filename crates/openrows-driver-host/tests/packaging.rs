// SPDX-License-Identifier: Apache-2.0
//! Installed native mismatch fails before Connect and removes every rendezvous endpoint.
use openrows_driver_api::DriverError;
use openrows_host_client::{DriverHostRuntime, HOST_BINARY_NAME, HostDriverId, HostFailure};

#[tokio::test]
async fn protocol_version_mismatch_is_typed_and_reaped() -> Result<(), Box<dyn std::error::Error>> {
    let root = if cfg!(unix) {
        "/tmp".into()
    } else {
        std::env::temp_dir()
    };
    let directory = tempfile::Builder::new().prefix(".v-").tempdir_in(root)?;
    let layout = directory.path().join("MacOS");
    std::fs::create_dir(&layout)?;
    std::fs::copy(
        env!("CARGO_BIN_EXE_host-mismatch-fixture"),
        layout.join(HOST_BINARY_NAME),
    )?;
    let endpoints = directory.path().join("hosts");
    let runtime = DriverHostRuntime::new(layout.join("openrows-app"), endpoints.clone());
    for _ in 0..3 {
        let launcher = runtime.launcher(HostDriverId::Probe, "test-only")?;
        assert!(matches!(
            launcher.spawn().await,
            Err(HostFailure::Incompatible)
        ));
    }
    let launcher = runtime.launcher(HostDriverId::Probe, "test-only")?;
    assert!(matches!(launcher.spawn().await, Err(HostFailure::Spawn)));
    let other = runtime.launcher(HostDriverId::Probe, "another-profile")?;
    assert!(matches!(
        other.spawn().await,
        Err(HostFailure::Incompatible)
    ));
    assert_eq!(
        DriverError::Host(HostFailure::Incompatible).to_string(),
        "driver host is incompatible with this app"
    );
    assert_eq!(std::fs::read_dir(endpoints)?.count(), 0);
    Ok(())
}

#[tokio::test]
async fn probe_has_no_driver_and_refuses_database_open() -> Result<(), Box<dyn std::error::Error>> {
    let root = if cfg!(unix) {
        "/tmp".into()
    } else {
        std::env::temp_dir()
    };
    let directory = tempfile::Builder::new().prefix(".p-").tempdir_in(root)?;
    let mut session =
        openrows_host_client::HostSupervisor::new(openrows_host_client::LaunchOptions {
            executable: env!("CARGO_BIN_EXE_openrows-driver-host").into(),
            driver: HostDriverId::Probe,
            endpoint_root: directory.path().to_path_buf(),
            memory: openrows_host_client::MemoryBudget::default(),
            circuit: openrows_host_client::CircuitBreaker::default(),
        })
        .spawn()
        .await?;
    let hello = openrows_host_protocol::Hello::for_driver(HostDriverId::Probe, 1);
    assert!(hello.capabilities.features.is_empty() && hello.capabilities.objects.is_empty());
    session.send(1, &openrows_host_protocol::Control::Connect(config()))?;
    tokio::time::timeout(std::time::Duration::from_secs(5), refused(&mut session)).await??;
    session.close().await?;
    Ok(())
}
async fn refused(
    session: &mut openrows_host_client::HostSession,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        match session.recv().await?.decode_control()? {
            openrows_host_protocol::Control::ConnectError {
                error: openrows_host_protocol::WireError::Unsupported(message),
                ..
            } => {
                assert_eq!(message, "handshake probe cannot open a database");
                return Ok(());
            }
            openrows_host_protocol::Control::Heartbeat
            | openrows_host_protocol::Control::Accepted => {}
            _ => return Err("unexpected probe Connect response".into()),
        }
    }
}
fn config() -> openrows_host_protocol::OwnedConnect {
    openrows_host_protocol::OwnedConnect {
        endpoint: openrows_host_protocol::WireEndpoint::File {
            source: openrows_host_protocol::WireFile::Memory {
                name: "test-only".into(),
            },
            attached: vec![],
        },
        database: "main".into(),
        user: String::new(),
        password: None,
        application: "test-only".into(),
        ssl_mode: 0,
        tls: openrows_host_protocol::WireTls::default(),
        timeouts_ms: [5000, 10000, 10000, 15000],
        startup: vec![],
        options: vec![],
    }
}
