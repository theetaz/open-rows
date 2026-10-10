// SPDX-License-Identifier: Apache-2.0
//! Private local sockets/pipes, never a TCP listener.
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
pub use unix::{Listener, Stream, configure_data, connect};
#[cfg(windows)]
pub use windows::{Listener, Stream, connect};
