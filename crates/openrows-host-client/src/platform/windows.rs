// SPDX-License-Identifier: Apache-2.0
//! Per-host Job Object memory cap and kill-on-parent-handle-close.
#![allow(
    unsafe_code,
    reason = "Job Object lifecycle and process memory use reviewed Win32 APIs"
)]
use crate::{HOST_MEMORY, HostFailure};
use std::io;
use tokio::process::Child;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        JobObjects::*,
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    },
};
pub(crate) struct Guard(HANDLE, &'static str);
// SAFETY: the owned job handle is only closed after its owning monitor finishes.
unsafe impl Send for Guard {}
impl Guard {
    pub(crate) fn attach(child: &Child) -> Result<Self, HostFailure> {
        // SAFETY: null attributes/name create an unnamed non-inheritable job.
        let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if job.is_null() {
            return Err(HostFailure::Spawn);
        }
        let guard = Self(job, "Job Object process cap + shared parent reservation");
        // SAFETY: all-zero fields are valid before setting the job limit flags.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
        limits.ProcessMemoryLimit = HOST_MEMORY;
        let process = child.raw_handle().ok_or(HostFailure::Exited)?;
        // SAFETY: valid job/child handles and a correctly sized initialized limit struct.
        let configured = unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                u32::try_from(size_of_val(&limits)).map_err(|_| HostFailure::Spawn)?,
            )
        };
        // SAFETY: both handles remain live until the monitor has reaped the child.
        if configured == 0 || unsafe { AssignProcessToJobObject(job, process) } == 0 {
            return Err(HostFailure::Spawn);
        }
        Ok(guard)
    }
    pub(crate) fn policy(&self) -> &'static str {
        self.1
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: unique owned handle returned by CreateJobObjectW.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub(crate) fn memory(pid: u32) -> io::Result<usize> {
    // SAFETY: opening only query/read rights for the identified child PID.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if handle.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a zeroed C memory-counter struct is valid as an output buffer.
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    // SAFETY: the buffer is correctly sized/aligned and the handle remains open.
    let result = unsafe {
        GetProcessMemoryInfo(
            handle,
            &mut counters,
            u32::try_from(size_of_val(&counters)).map_err(|_| io::ErrorKind::InvalidData)?,
        )
    };
    // SAFETY: the query handle is uniquely owned by this function.
    unsafe {
        CloseHandle(handle);
    }
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(counters.WorkingSetSize)
    }
}

/// Close the inherited bootstrap pipe and remove its process-standard handle.
/// # Errors
/// Invalid inherited handle or Win32 close failure.
pub fn close_bootstrap_stdin() -> io::Result<()> {
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::Console::{GetStdHandle, STD_INPUT_HANDLE, SetStdHandle},
    };
    // SAFETY: querying the current process's inherited standard input handle.
    let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // SAFETY: null removes this process's inherited standard input.
    let removed = unsafe { SetStdHandle(STD_INPUT_HANDLE, std::ptr::null_mut()) };
    // SAFETY: the inherited bootstrap pipe is owned by this helper.
    let closed = unsafe { CloseHandle(handle) };
    if removed == 0 || closed == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Check the identified child without treating an exited Windows process as an orphan.
/// # Errors
/// Process query failed for a reason other than a missing PID.
pub fn is_process_alive(pid: u32) -> io::Result<bool> {
    use windows_sys::Win32::{
        Foundation::ERROR_INVALID_PARAMETER,
        System::Threading::{GetExitCodeProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    // SAFETY: query-only access to an identified PID; no process mutation.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == i32::try_from(ERROR_INVALID_PARAMETER).ok() {
            Ok(false)
        } else {
            Err(error)
        };
    }
    let mut status = 0;
    // SAFETY: valid query handle and initialized DWORD output pointer.
    let queried = unsafe { GetExitCodeProcess(handle, &raw mut status) };
    let error = io::Error::last_os_error();
    // SAFETY: this query handle is uniquely owned by this function.
    unsafe {
        CloseHandle(handle);
    }
    if queried == 0 {
        Err(error)
    } else {
        Ok(status == 259)
    } // STILL_ACTIVE
}
