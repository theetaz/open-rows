// SPDX-License-Identifier: Apache-2.0
//! Private endpoints and authenticated lockstep startup, bounded to two seconds.
use crate::{
    HostFailure,
    transport::{Listener, Stream},
};
use openrows_host_protocol::{
    Authentication, Bootstrap, Control, Frame, Hello, Lane, ProtocolError, WireSecret, read_frame,
    write_frame,
};
use ring::rand::{SecureRandom, SystemRandom};
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zeroize::Zeroizing;
static GENERATION: AtomicU64 = AtomicU64::new(1);

pub(crate) struct Rendezvous {
    pub(crate) directory: TempDir,
    pub(crate) bootstrap: Bootstrap,
    control: Listener,
    data: Listener,
}
impl Rendezvous {
    pub(crate) fn new(root: &Path) -> Result<Self, HostFailure> {
        let directory = tempfile::Builder::new()
            .prefix(".h-")
            .tempdir_in(root)
            .map_err(|_| HostFailure::Spawn)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
                .map_err(|_| HostFailure::Spawn)?;
        }
        let token = random_token()?;
        let generation = GENERATION.fetch_add(1, Ordering::SeqCst);
        let endpoint_random = random_token()?;
        let control_path = endpoint(directory.path(), "c", &endpoint_random);
        let data_path = endpoint(directory.path(), "d", &endpoint_random);
        let control = Listener::bind(Path::new(&control_path)).map_err(|_| HostFailure::Spawn)?;
        let data = Listener::bind(Path::new(&data_path)).map_err(|_| HostFailure::Spawn)?;
        let bootstrap = Bootstrap {
            control: control_path,
            data: data_path,
            directory: directory.path().to_string_lossy().into_owned(),
            token: WireSecret::new(token),
            parent_generation: u64::from(std::process::id()),
            generation,
        };
        Ok(Self {
            directory,
            bootstrap,
            control,
            data,
        })
    }
    pub(crate) async fn attach(
        &mut self,
        driver: crate::HostDriverId,
    ) -> Result<(Stream, Stream), HostFailure> {
        let mut authentication = Authentication::new(self.bootstrap.clone());
        let mut control =
            authenticate(&mut self.control, &mut authentication, Lane::Control).await?;
        let data = authenticate(&mut self.data, &mut authentication, Lane::Data).await?;
        #[cfg(unix)]
        crate::transport::configure_data(&data).map_err(|_| HostFailure::Spawn)?;
        let hello = Hello::for_driver(driver, self.bootstrap.generation);
        let frame = Frame::control(hello.generation, 0, &Control::Hello(hello.clone()))
            .map_err(HostFailure::from)?;
        write_frame(&mut control, &frame, Lane::Control)
            .await
            .map_err(HostFailure::from)?;
        let ready = read_frame(&mut control, Lane::Control)
            .await
            .map_err(HostFailure::from)?;
        if ready.header.kind.0 != 3
            || ready.header.request != 0
            || ready.header.generation != hello.generation
        {
            return Err(HostFailure::Protocol);
        }
        let Control::Ready(ready) = ready.decode_control().map_err(HostFailure::from)? else {
            return Err(HostFailure::Protocol);
        };
        ready.validate(&hello).map_err(HostFailure::from)?;
        Ok((control, data))
    }
}
async fn authenticate(
    listener: &mut Listener,
    authentication: &mut Authentication,
    lane: Lane,
) -> Result<Stream, HostFailure> {
    loop {
        let mut peer = listener.accept().await.map_err(|_| HostFailure::Spawn)?;
        let candidate = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            read_frame(&mut peer, lane),
        )
        .await;
        let Ok(Ok(frame)) = candidate else {
            continue;
        };
        if frame.header.kind.0 != 1 {
            continue;
        }
        let Ok(Control::Authenticate(hello)) = frame.decode_control() else {
            continue;
        };
        if frame.header.request != 0
            || frame.header.generation != hello.generation
            || authentication.accept(&hello, lane).is_err()
        {
            continue;
        }
        let ack = Frame::control(hello.generation, 0, &Control::Authenticate(hello))
            .map_err(HostFailure::from)?;
        write_frame(&mut peer, &ack, lane)
            .await
            .map_err(HostFailure::from)?;
        return Ok(peer);
    }
}
fn random_token() -> Result<String, HostFailure> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = Zeroizing::new([0_u8; 32]);
    SystemRandom::new()
        .fill(bytes.as_mut())
        .map_err(|_| HostFailure::Spawn)?;
    let mut result = String::with_capacity(64);
    for byte in bytes.iter() {
        result.push(char::from(HEX[usize::from(byte >> 4)]));
        result.push(char::from(HEX[usize::from(byte & 15)]));
    }
    Ok(result)
}
fn endpoint(directory: &Path, name: &str, random: &str) -> String {
    #[cfg(unix)]
    {
        let _ = random;
        directory.join(name).to_string_lossy().into_owned()
    }
    #[cfg(windows)]
    {
        let _ = directory;
        format!(r"\\.\pipe\openrows-{random}-{name}")
    }
}
/// Write bounded inherited bootstrap bytes and close the parent stdin handle.
/// # Errors
/// Bootstrap over 8 KiB or ended child input.
pub async fn write_bootstrap(
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    bootstrap: &Bootstrap,
) -> Result<(), ProtocolError> {
    let bytes = Zeroizing::new(postcard_bytes(bootstrap)?);
    if bytes.len() > 8192 {
        return Err(ProtocolError::Limit);
    }
    writer
        .write_all(
            &u32::try_from(bytes.len())
                .map_err(|_| ProtocolError::Limit)?
                .to_le_bytes(),
        )
        .await
        .map_err(|_| ProtocolError::Transport)?;
    writer
        .write_all(&bytes)
        .await
        .map_err(|_| ProtocolError::Transport)?;
    writer
        .shutdown()
        .await
        .map_err(|_| ProtocolError::Transport)
}

/// Closed inherited input before Ready denotes an incompatible helper.
pub(crate) async fn bootstrap_host(
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    bootstrap: &Bootstrap,
) -> Result<(), HostFailure> {
    write_bootstrap(writer, bootstrap).await.map_err(|error| {
        // The child-exit and broken-input branches must agree before authentication.
        match error {
            ProtocolError::Transport => HostFailure::Incompatible,
            error => error.into(),
        }
    })
}
fn postcard_bytes(bootstrap: &Bootstrap) -> Result<Vec<u8>, ProtocolError> {
    openrows_host_protocol::encode_bootstrap(bootstrap)
}

/// Read stdin once; payload/frame lengths are validated before allocation.
/// # Errors
/// Missing token, malformed or oversized bootstrap.
pub async fn read_bootstrap(
    reader: &mut (impl tokio::io::AsyncRead + Unpin),
) -> Result<Bootstrap, ProtocolError> {
    let mut prefix = [0; 4];
    reader
        .read_exact(&mut prefix)
        .await
        .map_err(|_| ProtocolError::Truncated)?;
    let length = u32::from_le_bytes(prefix) as usize;
    if length == 0 || length > 8192 {
        return Err(ProtocolError::Limit);
    }
    let mut bytes = Zeroizing::new(vec![0; length]);
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|_| ProtocolError::Truncated)?;
    openrows_host_protocol::decode_bootstrap(&bytes)
}

#[cfg(test)]
mod tests;
