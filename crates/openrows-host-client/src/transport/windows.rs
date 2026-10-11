// SPDX-License-Identifier: Apache-2.0
//! Local-only named pipes; protected owner-only DACL uses the creator's token owner.
#![allow(
    unsafe_code,
    reason = "Win32 security descriptors require reviewed platform FFI"
)]
use std::{
    io,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions},
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::{
        Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, SECURITY_ATTRIBUTES,
    },
};

/// Both endpoint sides implement the same byte-stream interface.
pub enum Stream {
    /// Accepted local parent end.
    Server(NamedPipeServer),
    /// Connected helper end.
    Client(NamedPipeClient),
}
impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(stream) => Pin::new(stream).poll_read(context, buffer),
            Self::Client(stream) => Pin::new(stream).poll_read(context, buffer),
        }
    }
}
impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Server(stream) => Pin::new(stream).poll_write(context, buffer),
            Self::Client(stream) => Pin::new(stream).poll_write(context, buffer),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(stream) => Pin::new(stream).poll_flush(context),
            Self::Client(stream) => Pin::new(stream).poll_flush(context),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Server(stream) => Pin::new(stream).poll_shutdown(context),
            Self::Client(stream) => Pin::new(stream).poll_shutdown(context),
        }
    }
}
/// One private pipe instance; subsequent unauthenticated attempts get a fresh instance.
pub struct Listener {
    path: std::path::PathBuf,
    server: Option<NamedPipeServer>,
}
impl Listener {
    /// Restrict clients to the creator/owner and refuse remote machines.
    /// # Errors
    /// Pipe or security descriptor creation failed.
    pub fn bind(path: &Path) -> io::Result<Self> {
        Ok(Self {
            path: path.into(),
            server: Some(create(path, true)?),
        })
    }
    /// Attach one client; replay is rejected by the protocol authenticator.
    /// Cancelling acceptance preserves the pending pipe instance.
    /// # Errors
    /// Pipe connection failed.
    pub async fn accept(&mut self) -> io::Result<Stream> {
        if self.server.is_none() {
            self.server = Some(create(&self.path, false)?);
        }
        let server = self.server.as_ref().ok_or(io::ErrorKind::NotConnected)?;
        server.connect().await?;
        self.server
            .take()
            .map(Stream::Server)
            .ok_or_else(|| io::ErrorKind::NotConnected.into())
    }
}
fn create(path: &Path, first: bool) -> io::Result<NamedPipeServer> {
    let sddl: Vec<u16> = "D:P(A;;GA;;;OW)\0".encode_utf16().collect();
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: NUL-terminated UTF-16 input and a valid descriptor output pointer.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>())
            .map_err(|_| io::ErrorKind::InvalidData)?,
        lpSecurityDescriptor: descriptor,
        bInheritHandle: 0,
    };
    // SAFETY: the descriptor and attributes remain live through synchronous pipe creation.
    let result = unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(path, (&raw mut attributes).cast())
    };
    // SAFETY: this descriptor was allocated by the conversion API with LocalAlloc.
    unsafe {
        LocalFree(descriptor);
    }
    result
}
/// Connect without a shell, TCP fallback or remote pipe acceptance.
/// # Errors
/// Missing or inaccessible pipe.
pub async fn connect(path: &Path) -> io::Result<Stream> {
    ClientOptions::new().open(path).map(Stream::Client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn cancelling_accept_preserves_the_pipe_for_a_later_client()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::Builder::new()
            .prefix("openrows-accept-")
            .tempdir()?;
        let name = directory
            .path()
            .file_name()
            .ok_or(io::ErrorKind::InvalidInput)?;
        let path = std::path::PathBuf::from(format!(r"\\.\pipe\{}", name.to_string_lossy()));
        let mut listener = Listener::bind(&path)?;
        {
            let pending = listener.accept();
            tokio::pin!(pending);
            tokio::select! {
                biased;
                result = &mut pending => return Err(format!("accept completed without a client: {}", result.is_ok()).into()),
                () = tokio::task::yield_now() => {},
            }
        }
        let mut client = connect(&path).await?;
        let mut server =
            tokio::time::timeout(crate::HANDSHAKE_TIMEOUT, listener.accept()).await??;
        client.write_all(b"x").await?;
        assert_eq!(server.read_u8().await?, b'x');
        Ok(())
    }
}
