// SPDX-License-Identifier: Apache-2.0
//! cgroup v2 when delegated; otherwise the accepted 100 ms RSS monitor.
use crate::{HOST_MEMORY, HostFailure};
use std::{io, path::PathBuf};
use tokio::process::Child;
pub(crate) struct Guard(Option<PathBuf>);
impl Guard {
    pub(crate) fn attach(child: &Child) -> Result<Self, HostFailure> {
        let pid = child.id().ok_or(HostFailure::Exited)?;
        let path = delegated_group(pid);
        if let Some(path) = &path
            && (std::fs::write(path.join("memory.max"), HOST_MEMORY.to_string()).is_err()
                || std::fs::write(path.join("cgroup.procs"), pid.to_string()).is_err())
        {
            let _ = std::fs::remove_dir(path);
            return Ok(Self(None));
        }
        Ok(Self(path))
    }
    pub(crate) fn policy(&self) -> &'static str {
        if self.0.is_some() {
            "cgroup-v2 + monitoring"
        } else {
            "monitoring-only fallback (cgroup v2 not delegated)"
        }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = std::fs::remove_dir(path);
        }
    }
}
fn delegated_group(pid: u32) -> Option<PathBuf> {
    let groups = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let current = groups.lines().find_map(|line| line.strip_prefix("0::"))?;
    let root = PathBuf::from("/sys/fs/cgroup").join(current.trim_start_matches('/'));
    let path = root.join(format!("openrows-{}-{pid}", std::process::id()));
    std::fs::create_dir(&path).ok()?;
    Some(path)
}
pub(crate) fn memory(pid: u32) -> io::Result<usize> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status"))?;
    resident_bytes(&status)
}
fn resident_bytes(status: &str) -> io::Result<usize> {
    let kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or(io::ErrorKind::InvalidData)?;
    kib.checked_mul(1024)
        .ok_or_else(|| io::ErrorKind::InvalidData.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rss_samples_preserve_read_and_parse_errors() {
        assert!(memory(std::process::id()).is_ok());
        assert_eq!(
            memory(u32::MAX).err().map(|error| error.kind()),
            Some(io::ErrorKind::NotFound)
        );
        for status in [
            "State:\tZ (zombie)\n",
            "VmRSS:\tinvalid kB\n",
            "VmRSS:\t18446744073709551615 kB\n",
        ] {
            assert_eq!(
                resident_bytes(status).err().map(|error| error.kind()),
                Some(io::ErrorKind::InvalidData)
            );
        }
        assert_eq!(resident_bytes("VmRSS:\t42 kB\n").ok(), Some(42 * 1024));
    }
}
