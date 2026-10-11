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

#[tokio::test(start_paused = true)]
async fn authentication_frame_survives_a_scheduling_gap_within_the_handshake_deadline()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = endpoint_root()?;
    let rendezvous = Rendezvous::new(directory.path())?;
    let (mut peer, mut accepted) = tokio::io::duplex(1024);
    let frame = authentication_frame(&rendezvous.bootstrap, rendezvous.bootstrap.token.expose())?;
    let bytes = frame.encode(Lane::Control)?;
    peer.write_all(&bytes[..1]).await?;
    let pending = tokio::time::timeout(
        crate::HANDSHAKE_TIMEOUT,
        read_candidate(&mut accepted, Lane::Control),
    );
    tokio::pin!(pending);
    tokio::select! {
        biased;
        result = &mut pending => return Err(format!("authentication completed early: {}", result.is_ok()).into()),
        () = tokio::task::yield_now() => {},
    }
    tokio::time::advance(std::time::Duration::from_millis(150)).await;
    tokio::select! {
        biased;
        result = &mut pending => return Err(format!("authentication completed early: {}", result.is_ok()).into()),
        () = tokio::task::yield_now() => {},
    }
    peer.write_all(&bytes[1..]).await?;
    let received = pending.await??;
    assert_eq!(received.decode_control()?, frame.decode_control()?);
    Ok(())
}

fn authentication_frame(bootstrap: &Bootstrap, token: &str) -> Result<Frame, ProtocolError> {
    Frame::control(
        bootstrap.generation,
        0,
        &Control::Authenticate(openrows_host_protocol::LaneHello {
            token: WireSecret::new(token.into()),
            lane: Lane::Control,
            parent_generation: bootstrap.parent_generation,
            generation: bootstrap.generation,
        }),
    )
}

#[cfg(unix)]
#[tokio::test]
async fn idle_and_wrong_token_peers_do_not_block_the_authenticated_child()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = endpoint_root()?;
    let mut rendezvous = Rendezvous::new(directory.path())?;
    let path = Path::new(&rendezvous.bootstrap.control);
    let mut idle = crate::transport::connect(path).await?;
    let mut wrong = crate::transport::connect(path).await?;
    let mut valid = crate::transport::connect(path).await?;
    write_frame(
        &mut wrong,
        &authentication_frame(&rendezvous.bootstrap, "wrong")?,
        Lane::Control,
    )
    .await?;
    let frame = authentication_frame(&rendezvous.bootstrap, rendezvous.bootstrap.token.expose())?;
    write_frame(&mut valid, &frame, Lane::Control).await?;
    let mut authentication = Authentication::new(rendezvous.bootstrap.clone());
    let _attached = tokio::time::timeout(
        crate::HANDSHAKE_TIMEOUT,
        authenticate(&mut rendezvous.control, &mut authentication, Lane::Control),
    )
    .await??;
    assert_eq!(
        read_frame(&mut valid, Lane::Control)
            .await?
            .decode_control()?,
        frame.decode_control()?
    );
    assert_eq!(idle.read(&mut [0]).await?, 0);
    assert_eq!(wrong.read(&mut [0]).await?, 0);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn a_silent_authentication_frame_still_hits_the_handshake_deadline() {
    let (_peer, mut accepted) = tokio::io::duplex(64);
    let pending = tokio::time::timeout(
        crate::HANDSHAKE_TIMEOUT,
        read_candidate(&mut accepted, Lane::Control),
    );
    tokio::pin!(pending);
    tokio::select! {
        biased;
        result = &mut pending => assert!(result.is_err(), "silent peer completed before its deadline"),
        () = tokio::task::yield_now() => {},
    }
    tokio::time::advance(crate::HANDSHAKE_TIMEOUT).await;
    assert!(pending.await.is_err());
}
