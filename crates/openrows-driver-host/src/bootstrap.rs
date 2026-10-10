// SPDX-License-Identifier: Apache-2.0
//! Bootstrap is read once, zeroized after both lanes authenticate, and never logged.
use openrows_host_client::{
    HANDSHAKE_TIMEOUT, read_bootstrap,
    transport::{Stream, connect},
};
use openrows_host_protocol::{
    Authentication, Bootstrap, Control, Frame, Hello, Lane, LaneHello, ProtocolError, read_frame,
    write_frame,
};
use std::path::Path;

struct Directory(String);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub(crate) async fn run() -> Result<(), ProtocolError> {
    if std::env::args().nth(1).as_deref() != Some("sqlite") {
        return Err(ProtocolError::Incompatible);
    }
    let bootstrap =
        tokio::time::timeout(HANDSHAKE_TIMEOUT, read_bootstrap(&mut tokio::io::stdin()))
            .await
            .map_err(|_| ProtocolError::Authentication)??;
    let _directory = Directory(bootstrap.directory.clone());
    close_stdin()?;
    let (control, data) = tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake(&bootstrap))
        .await
        .map_err(|_| ProtocolError::Authentication)??;
    let generation = bootstrap.generation;
    drop(bootstrap);
    crate::control::run(control, data, generation).await
}
async fn handshake(bootstrap: &Bootstrap) -> Result<(Stream, Stream), ProtocolError> {
    let mut control = connect(Path::new(&bootstrap.control))
        .await
        .map_err(|_| ProtocolError::Transport)?;
    let mut data = connect(Path::new(&bootstrap.data))
        .await
        .map_err(|_| ProtocolError::Transport)?;
    let mut authentication = Authentication::new(bootstrap.clone());
    authenticate(&mut control, bootstrap, &mut authentication, Lane::Control).await?;
    authenticate(&mut data, bootstrap, &mut authentication, Lane::Data).await?;
    #[cfg(unix)]
    openrows_host_client::transport::configure_data(&data).map_err(|_| ProtocolError::Transport)?;
    let hello = read_frame(&mut control, Lane::Control).await?;
    if hello.header.kind.0 != 2
        || hello.header.generation != bootstrap.generation
        || hello.header.request != 0
    {
        return Err(ProtocolError::Stale);
    }
    let Control::Hello(hello) = hello.decode_control()? else {
        return Err(ProtocolError::Authentication);
    };
    let contract = Hello::sqlite(bootstrap.generation);
    hello.validate(&contract)?;
    let ready = Frame::control(bootstrap.generation, 0, &Control::Ready(contract))?;
    write_frame(&mut control, &ready, Lane::Control).await?;
    Ok((control, data))
}
async fn authenticate(
    stream: &mut Stream,
    bootstrap: &Bootstrap,
    authentication: &mut Authentication,
    lane: Lane,
) -> Result<(), ProtocolError> {
    let auth = Control::Authenticate(LaneHello {
        token: bootstrap.token.clone(),
        lane,
        parent_generation: bootstrap.parent_generation,
        generation: bootstrap.generation,
    });
    write_frame(
        stream,
        &Frame::control(bootstrap.generation, 0, &auth)?,
        lane,
    )
    .await?;
    let ack = read_frame(stream, lane).await?;
    if ack.header.kind.0 != 1
        || ack.header.request != 0
        || ack.header.generation != bootstrap.generation
    {
        return Err(ProtocolError::Stale);
    }
    let Control::Authenticate(hello) = ack.decode_control()? else {
        return Err(ProtocolError::Authentication);
    };
    authentication.accept(&hello, lane)
}
fn close_stdin() -> Result<(), ProtocolError> {
    #[cfg(unix)]
    {
        let null = rustix::fs::open(
            "/dev/null",
            rustix::fs::OFlags::RDONLY,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| ProtocolError::Transport)?;
        rustix::stdio::dup2_stdin(&null).map_err(|_| ProtocolError::Transport)?;
    }
    #[cfg(windows)]
    openrows_host_client::close_bootstrap_stdin().map_err(|_| ProtocolError::Transport)?;
    Ok(())
}
