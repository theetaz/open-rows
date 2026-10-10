// SPDX-License-Identifier: Apache-2.0
use super::*;

fn endpoint_root() -> Result<TempDir, std::io::Error> {
    tempfile::Builder::new()
        .prefix(".b-")
        .tempdir_in(if cfg!(unix) {
            std::path::PathBuf::from("/tmp")
        } else {
            std::env::temp_dir()
        })
}

#[tokio::test]
async fn closed_bootstrap_input_is_incompatible_before_ready()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = endpoint_root()?;
    let rendezvous = Rendezvous::new(directory.path())?;
    let (mut input, closed) = tokio::io::duplex(64);
    drop(closed);
    assert_eq!(
        bootstrap_host(&mut input, &rendezvous.bootstrap).await,
        Err(HostFailure::Incompatible)
    );
    Ok(())
}

#[tokio::test]
async fn invalid_parent_bootstrap_remains_a_protocol_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = endpoint_root()?;
    let mut rendezvous = Rendezvous::new(directory.path())?;
    rendezvous.bootstrap.control = "x".repeat(8193);
    assert_eq!(
        bootstrap_host(&mut tokio::io::sink(), &rendezvous.bootstrap).await,
        Err(HostFailure::Protocol)
    );
    Ok(())
}
