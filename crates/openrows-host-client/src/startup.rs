// SPDX-License-Identifier: Apache-2.0
//! Drain child output while retaining only bounded, allowlisted startup categories.
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    task::JoinHandle,
};

pub(crate) struct Diagnostics {
    task: Option<JoinHandle<()>>,
    reason: Arc<Mutex<Option<String>>>,
}
impl Diagnostics {
    pub(crate) fn start(reader: impl AsyncRead + Unpin + Send + 'static) -> Self {
        let reason = Arc::default();
        Self {
            task: Some(tokio::spawn(read(reader, Arc::clone(&reason)))),
            reason,
        }
    }
    pub(crate) fn into_task(mut self) -> Option<JoinHandle<()>> {
        self.task.take()
    }
    pub(crate) async fn finish(mut self) -> Option<String> {
        // An unrelated executable can leave descendants holding stderr open.
        let mut task = self.task.take()?;
        if tokio::time::timeout(crate::CLOSE_GRACE, &mut task)
            .await
            .is_err()
        {
            task.abort();
            let _ = task.await;
        }
        self.reason.lock().ok()?.take()
    }
}
impl Drop for Diagnostics {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
pub(crate) async fn failure(
    error: crate::HostFailure,
    driver: crate::HostDriverId,
    exit: Option<Result<std::process::ExitStatus, std::io::Error>>,
    diagnostic: Option<Diagnostics>,
) {
    let helper_reason = match diagnostic {
        Some(diagnostic) => diagnostic.finish().await,
        None => None,
    };
    tracing::warn!(check = "startup_failed", driver = driver.argument(),
        failure = ?error, exit = ?exit.as_ref().and_then(|status| status.as_ref().ok()),
        wait_error = ?exit.as_ref().and_then(|status| status.as_ref().err()).map(std::io::Error::kind), helper_reason,
        "driver host startup failed before Ready");
}
async fn read(mut reader: impl AsyncRead + Unpin, reason: Arc<Mutex<Option<String>>>) {
    let mut buffer = [0; 4096];
    let mut line = Vec::with_capacity(128);
    let mut oversized = false;
    while let Ok(count @ 1..) = reader.read(&mut buffer).await {
        for byte in &buffer[..count] {
            if *byte == b'\n' {
                record(&line, oversized, &reason);
                line.clear();
                oversized = false;
            } else if line.len() < 128 && !oversized {
                line.push(*byte);
            } else {
                oversized = true;
            }
        }
    }
}
fn record(line: &[u8], oversized: bool, reason: &Mutex<Option<String>>) {
    if oversized {
        return;
    }
    if let Some(category) = category(line)
        && let Ok(mut reason) = reason.lock()
        && reason.is_none()
    {
        *reason = Some(category);
    }
}
fn category(line: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(line).ok()?;
    let (stage, error) = text
        .strip_prefix("OPENROWS_HOST_STARTUP:")?
        .split_once(':')?;
    if !matches!(
        stage,
        "driver_argument"
            | "sqlite_feature"
            | "bootstrap_read"
            | "stdin_close"
            | "handshake"
            | "control_connect"
            | "data_connect"
            | "control_authenticate"
            | "data_authenticate"
            | "data_configure"
            | "hello_read"
            | "hello_header"
            | "hello_decode"
            | "hello_kind"
            | "hello_contract"
            | "hello_version"
            | "hello_schema"
            | "hello_build"
            | "hello_driver"
            | "hello_generation"
            | "hello_limits"
            | "hello_arrow"
            | "hello_required"
            | "hello_capabilities"
            | "ready_encode"
            | "ready_write"
    ) || !matches!(
        error,
        "Truncated"
            | "Limit"
            | "UnknownKind"
            | "Invalid"
            | "Stale"
            | "Incompatible"
            | "Authentication"
            | "Transport"
    ) {
        return None;
    }
    Some(format!("{stage}:{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn startup_capture_discards_payloads_and_preserves_first_precise_category() {
        let bytes = b"native test-only payload\nOPENROWS_HOST_STARTUP:control_authenticate:Transport\nOPENROWS_HOST_STARTUP:handshake:Transport\n";
        let reason = Arc::default();
        read(&bytes[..], Arc::clone(&reason)).await;
        assert_eq!(
            reason
                .lock()
                .ok()
                .and_then(|reason| reason.clone())
                .as_deref(),
            Some("control_authenticate:Transport")
        );
    }

    #[tokio::test]
    async fn startup_capture_rejects_unknown_and_oversized_lines() {
        let mut bytes = vec![b'x'; 1024];
        bytes.extend_from_slice(b"OPENROWS_HOST_STARTUP:hello_read:Incompatible\nOPENROWS_HOST_STARTUP:hello_read:test-only payload\nOPENROWS_HOST_STARTUP:test-only:Transport\n");
        let reason = Arc::default();
        read(&bytes[..], Arc::clone(&reason)).await;
        assert_eq!(reason.lock().ok().and_then(|reason| reason.clone()), None);
    }

    #[tokio::test(start_paused = true)]
    async fn startup_capture_bounds_a_reader_held_open_after_child_exit() {
        let (_descendant, reader) = tokio::io::duplex(64);
        assert_eq!(Diagnostics::start(reader).finish().await, None);
    }
}
