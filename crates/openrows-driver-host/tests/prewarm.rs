// SPDX-License-Identifier: Apache-2.0
//! Installed Probe warms once without database access or profile circuit effects.
use openrows_host_client::{
    DriverHostRuntime, HOST_BINARY_NAME, HOST_MEMORY, HostDriverId, HostFailure, MemoryBudget,
    PrewarmOutcome,
};

fn installed(
    helper: &str,
) -> Result<(tempfile::TempDir, DriverHostRuntime), Box<dyn std::error::Error>> {
    let root = if cfg!(unix) {
        "/tmp".into()
    } else {
        std::env::temp_dir()
    };
    let directory = tempfile::Builder::new().prefix(".p-").tempdir_in(root)?;
    let layout = directory.path().join("MacOS");
    std::fs::create_dir(&layout)?;
    std::fs::copy(helper, layout.join(HOST_BINARY_NAME))?;
    let runtime =
        DriverHostRuntime::new(layout.join("openrows-app"), directory.path().join("hosts"));
    Ok((directory, runtime))
}

#[tokio::test]
async fn installed_probe_prewarm_closes_once_and_releases_budget()
-> Result<(), Box<dyn std::error::Error>> {
    let (directory, runtime) = installed(env!("CARGO_BIN_EXE_host-test-fixture"))?;
    let memory = MemoryBudget::default();
    assert_eq!(memory.reserved(), 0);
    let report = runtime.prewarm().await.ok_or("prewarm report")?;
    assert_eq!(report.outcome, PrewarmOutcome::Ready);
    assert!(report.handshake.is_some());
    assert_eq!(memory.reserved(), 0);
    assert!(runtime.prewarm().await.is_none());
    assert_eq!(runtime.prewarm_report(), Some(report));
    assert_eq!(
        std::fs::read_dir(directory.path().join("hosts"))?.count(),
        0
    );
    let host = runtime
        .launcher(HostDriverId::Duckdb, "test-only")?
        .spawn()
        .await?;
    host.close().await?;
    Ok(())
}

#[tokio::test]
async fn prewarm_respects_full_memory_admission_and_leaves_profile_circuit_untouched()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, runtime) = installed(env!("CARGO_BIN_EXE_host-mismatch-fixture"))?;
    let memory = MemoryBudget::default();
    let first = memory.host()?;
    let second = memory.host()?;
    let report = runtime.prewarm().await.ok_or("prewarm report")?;
    assert_eq!(
        report.outcome,
        PrewarmOutcome::Failed(HostFailure::MemoryLimit)
    );
    assert_eq!(memory.reserved(), 2 * HOST_MEMORY);
    drop((first, second));
    // Three real profile launches still reach the mismatch instead of a tripped circuit.
    for _ in 0..3 {
        assert!(matches!(
            runtime
                .launcher(HostDriverId::Duckdb, "test-only")?
                .spawn()
                .await,
            Err(HostFailure::Incompatible)
        ));
    }
    assert!(matches!(
        runtime
            .launcher(HostDriverId::Duckdb, "test-only")?
            .spawn()
            .await,
        Err(HostFailure::Spawn)
    ));
    Ok(())
}

#[tokio::test]
async fn failed_prewarm_handshake_is_recorded_without_consuming_a_profile_exit()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, runtime) = installed(env!("CARGO_BIN_EXE_host-mismatch-fixture"))?;
    let report = runtime.prewarm().await.ok_or("prewarm report")?;
    assert_eq!(
        report.outcome,
        PrewarmOutcome::Failed(HostFailure::Incompatible)
    );
    assert_eq!(runtime.prewarm_report(), Some(report));
    assert!(runtime.prewarm().await.is_none());
    for _ in 0..3 {
        assert!(matches!(
            runtime
                .launcher(HostDriverId::Duckdb, "test-only")?
                .spawn()
                .await,
            Err(HostFailure::Incompatible)
        ));
    }
    Ok(())
}
