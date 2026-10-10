// SPDX-License-Identifier: Apache-2.0
//! Native test-only peer authenticates correctly but returns a different protocol version.
use openrows_host_client::{
    read_bootstrap,
    transport::{Stream, connect},
};
use openrows_host_protocol::{
    Bootstrap, Control, Frame, Hello, HostDriverId, Lane, LaneHello, ProtocolError, read_frame,
    write_frame,
};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap = read_bootstrap(&mut tokio::io::stdin()).await?;
    let mut control = lane(&bootstrap, Lane::Control, &bootstrap.control).await?;
    let _data = lane(&bootstrap, Lane::Data, &bootstrap.data).await?;
    let frame = read_frame(&mut control, Lane::Control).await?;
    let Control::Hello(_) = frame.decode_control()? else {
        return Err(ProtocolError::Invalid.into());
    };
    let mut hello = Hello::for_driver(HostDriverId::Probe, bootstrap.generation);
    hello.version.0 += 1;
    write_frame(
        &mut control,
        &Frame::control(bootstrap.generation, 0, &Control::Ready(hello))?,
        Lane::Control,
    )
    .await?;
    // Keep the lane alive until the parent terminates/reaps this incompatible peer.
    let _ = read_frame(&mut control, Lane::Control).await;
    Ok(())
}
async fn lane(bootstrap: &Bootstrap, lane: Lane, endpoint: &str) -> Result<Stream, ProtocolError> {
    let mut stream = connect(Path::new(endpoint))
        .await
        .map_err(|_| ProtocolError::Transport)?;
    let auth = Control::Authenticate(LaneHello {
        token: bootstrap.token.clone(),
        lane,
        parent_generation: bootstrap.parent_generation,
        generation: bootstrap.generation,
    });
    write_frame(
        &mut stream,
        &Frame::control(bootstrap.generation, 0, &auth)?,
        lane,
    )
    .await?;
    read_frame(&mut stream, lane).await?;
    Ok(stream)
}
