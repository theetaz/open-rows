// SPDX-License-Identifier: Apache-2.0
use std::{io, os::unix::fs::PermissionsExt, path::Path};
/// Local stream on macOS/Linux.
pub type Stream = tokio::net::UnixStream;
/// Private per-host listener; dropped as soon as authenticated child attaches.
pub struct Listener(tokio::net::UnixListener);
impl Listener {
    /// Bind with access restricted to the current user.
    /// # Errors
    /// Endpoint creation/permission failure.
    pub fn bind(path: &Path) -> io::Result<Self> {
        let listener = tokio::net::UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self(listener))
    }
    /// Accept one peer; authentication is separate and bounded.
    /// # Errors
    /// Local accept failed.
    pub async fn accept(&mut self) -> io::Result<Stream> {
        self.0.accept().await.map(|(stream, _)| stream)
    }
}
/// Connect only to the inherited private endpoint.
/// # Errors
/// Missing or inaccessible socket.
pub async fn connect(path: &Path) -> io::Result<Stream> {
    Stream::connect(path).await
}

/// Bound local data-socket buffers to 512 KiB per direction, avoiding 8 KiB macOS defaults.
/// # Errors
/// OS socket-buffer configuration failed.
pub fn configure_data(stream: &Stream) -> io::Result<()> {
    let socket = socket2::SockRef::from(stream);
    socket.set_send_buffer_size(512 * 1024)?;
    socket.set_recv_buffer_size(512 * 1024)
}
