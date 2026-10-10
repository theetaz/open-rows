// SPDX-License-Identifier: Apache-2.0
//! macOS accepted policy: 100 ms RSS monitoring with targeted force-kill.
#![allow(unsafe_code, reason = "libproc requires one reviewed OS FFI call")]
use crate::HostFailure;
use std::io;
use tokio::process::Child;

pub(crate) struct Guard(&'static str);
impl Guard {
    pub(crate) fn attach(child: &Child) -> Result<Self, HostFailure> {
        child.id().ok_or(HostFailure::Exited)?;
        Ok(Self("monitoring-only fallback (macOS)"))
    }
    pub(crate) fn policy(&self) -> &'static str {
        self.0
    }
}
impl Drop for Guard {
    fn drop(&mut self) {}
}
pub(crate) fn memory(pid: u32) -> io::Result<usize> {
    let pid = i32::try_from(pid).map_err(|_| io::ErrorKind::InvalidInput)?;
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let size = size_of::<libc::proc_taskinfo>();
    // SAFETY: libproc writes at most size bytes into correctly sized/aligned taskinfo.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            i32::try_from(size).map_err(|_| io::ErrorKind::InvalidInput)?,
        )
    };
    if usize::try_from(written).ok() != Some(size) {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful full-size proc_pidinfo call initialized every field.
    let info = unsafe { info.assume_init() };
    usize::try_from(info.pti_resident_size).map_err(|_| io::ErrorKind::InvalidData.into())
}
