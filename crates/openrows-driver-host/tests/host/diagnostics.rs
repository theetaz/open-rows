// SPDX-License-Identifier: Apache-2.0
//! Compatibility logs identify their check without exposing untrusted peer text.
use openrows_host_client::{HostFailure, HostSupervisor, MemoryBudget};
use openrows_host_protocol::Hello;
use std::{
    io::Write,
    sync::{Arc, Mutex},
};
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn capture(
    output: &Output,
) -> tracing_subscriber::fmt::Subscriber<
    tracing_subscriber::fmt::format::DefaultFields,
    tracing_subscriber::fmt::format::Format<tracing_subscriber::fmt::format::Full, ()>,
    tracing_subscriber::filter::LevelFilter,
    impl for<'a> tracing_subscriber::fmt::MakeWriter<'a> + use<>,
> {
    let writer = output.clone();
    tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_writer(move || writer.clone())
        .finish()
}

#[test]
fn contract_diagnostics_name_mismatches_and_omit_peer_strings() {
    let output = Output::default();
    let subscriber = capture(&output);
    let _guard = subscriber.set_default();
    let expected = Hello::sqlite(1);
    let mut peer = expected.clone();
    peer.driver = "test-only peer payload".into();
    peer.schema = "test-only schema payload".into();
    peer.build = "test-only build payload".into();
    peer.version.0 += 1;
    assert!(peer.validate(&expected).is_err());
    let log = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    for check in [
        "hello_version",
        "hello_schema",
        "hello_build",
        "hello_driver",
    ] {
        assert!(log.contains(check), "missing diagnostic {check}: {log}");
    }
    assert!(log.contains("observed=(2, 0) expected=(1, 0)"), "{log}");
    assert!(!log.contains("payload"), "{log}");
}

#[tokio::test]
async fn mismatched_ready_preserves_typed_failure_and_reports_precise_check() {
    let _permit = crate::support::host_test().await.unwrap();
    let output = Output::default();
    let subscriber = capture(&output);
    let _guard = subscriber.set_default();
    let mut options = crate::support::options(MemoryBudget::default());
    options.executable = env!("CARGO_BIN_EXE_host-mismatch-fixture").into();
    assert!(matches!(
        HostSupervisor::new(options).spawn().await,
        Err(HostFailure::Incompatible)
    ));
    let log = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    assert!(log.contains("hello_version"), "{log}");
    assert!(log.contains("startup_failed"), "{log}");
    assert!(!log.contains('/'), "path leaked into diagnostic: {log}");
}

#[tokio::test]
async fn fixture_reports_invalid_driver_before_bootstrap_without_echoing_argument() {
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_host-test-fixture"))
        .arg("test-only invalid payload")
        .env_clear()
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(70));
    assert_eq!(
        output.stderr,
        b"OPENROWS_HOST_STARTUP:driver_argument:Incompatible\n"
    );
}
