// SPDX-License-Identifier: Apache-2.0
//! Bootstrap is read once, zeroized after both lanes authenticate, and never logged.
use crate::startup::stage;
use openrows_host_client::{
    HANDSHAKE_TIMEOUT, read_bootstrap,
    transport::{Stream, connect},
};
use openrows_host_protocol::{
    Authentication, Bootstrap, Control, Frame, Hello, HostDriverId, Lane, LaneHello, ProtocolError,
    read_frame, write_frame,
};
use std::path::Path;

struct Directory(String);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub(crate) async fn run(connect: crate::registry::Connect) -> Result<(), ProtocolError> {
    let driver = stage(
        "driver_argument",
        std::env::args()
            .nth(1)
            .and_then(|argument| HostDriverId::parse(&argument))
            .ok_or(ProtocolError::Incompatible),
    )?;
    if driver == HostDriverId::SqliteTest && !cfg!(feature = "test-sqlite") {
        return stage("sqlite_feature", Err(ProtocolError::Incompatible));
    }
    let bootstrap = stage(
        "bootstrap_read",
        tokio::time::timeout(HANDSHAKE_TIMEOUT, read_bootstrap(&mut tokio::io::stdin()))
            .await
            .map_err(|_| ProtocolError::Authentication)
            .and_then(std::convert::identity),
    )?;
    let _directory = Directory(bootstrap.directory.clone());
    stage("stdin_close", close_stdin())?;
    let (control, data) = stage(
        "handshake",
        tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake(&bootstrap, driver))
            .await
            .map_err(|_| ProtocolError::Authentication)
            .and_then(std::convert::identity),
    )?;
    let generation = bootstrap.generation;
    drop(bootstrap);
    crate::control::run(
        control,
        data,
        generation,
        crate::registry::Registry { driver, connect },
    )
    .await
}
async fn handshake(
    bootstrap: &Bootstrap,
    driver: HostDriverId,
) -> Result<(Stream, Stream), ProtocolError> {
    let mut control = stage(
        "control_connect",
        connect(Path::new(&bootstrap.control))
            .await
            .map_err(|_| ProtocolError::Transport),
    )?;
    let mut data = stage(
        "data_connect",
        connect(Path::new(&bootstrap.data))
            .await
            .map_err(|_| ProtocolError::Transport),
    )?;
    let mut authentication = Authentication::new(bootstrap.clone());
    stage(
        "control_authenticate",
        authenticate(&mut control, bootstrap, &mut authentication, Lane::Control).await,
    )?;
    stage(
        "data_authenticate",
        authenticate(&mut data, bootstrap, &mut authentication, Lane::Data).await,
    )?;
    #[cfg(unix)]
    stage(
        "data_configure",
        openrows_host_client::transport::configure_data(&data)
            .map_err(|_| ProtocolError::Transport),
    )?;
    ready(&mut control, bootstrap.generation, driver).await?;
    Ok((control, data))
}
async fn ready(
    control: &mut Stream,
    generation: u64,
    driver: HostDriverId,
) -> Result<(), ProtocolError> {
    let hello = stage("hello_read", read_frame(control, Lane::Control).await)?;
    if hello.header.kind.0 != 2
        || hello.header.generation != generation
        || hello.header.request != 0
    {
        return stage("hello_header", Err(ProtocolError::Stale));
    }
    let Control::Hello(hello) = stage("hello_decode", hello.decode_control())? else {
        return stage("hello_kind", Err(ProtocolError::Authentication));
    };
    let contract = Hello::for_driver(driver, generation);
    crate::startup::contract(&hello, &contract)?;
    let ready = stage(
        "ready_encode",
        Frame::control(generation, 0, &Control::Ready(contract)),
    )?;
    stage(
        "ready_write",
        write_frame(control, &ready, Lane::Control).await,
    )?;
    Ok(())
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
