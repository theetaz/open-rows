// SPDX-License-Identifier: Apache-2.0
//! OS lifecycle policy; portable monitoring is explicit when kernel caps lack delegation.
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
pub(crate) use linux::{Guard, memory};
#[cfg(target_os = "macos")]
pub(crate) use macos::{Guard, memory};
#[cfg(windows)]
pub(crate) use windows::{Guard, memory};

#[cfg(windows)]
pub use windows::close_bootstrap_stdin;

#[cfg(windows)]
pub use windows::is_process_alive;
