// SPDX-License-Identifier: Apache-2.0
//! Golden templates and actual direct-spawn literal argument verification.
use openrows_net::helper::{HelperHop, HelperSpec, Helpers};
use std::{path::PathBuf, time::Duration};
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn hop(args: &[&str]) -> HelperHop {
    HelperHop::Custom {
        program: env!("CARGO_BIN_EXE_fake-cloud-hop").into(),
        args: args.iter().map(|arg| (*arg).into()).collect(),
        ready_timeout_seconds: None,
    }
}
#[test]
fn templates_substitute_each_argument_once_without_shell_expansion() -> TestResult {
    let hop = hop(&[
        "a;b $(x) {local_port}",
        "{remote_host}:{remote_port}",
        "{local_port}/{local_port}",
    ]);
    assert_eq!(
        hop.arguments_for(1234, "db{local_port}", 6432)?,
        ["a;b $(x) 1234", "db{local_port}:6432", "1234/1234"]
    );
    assert_eq!(
        HelperSpec::new(hop, PathBuf::new(), String::new()).timeout,
        Duration::from_secs(30)
    );
    Ok(())
}
#[test]
fn relative_program_missing_local_unknown_placeholder_and_timeout_are_rejected() {
    let mut relative = hop(&["{local_port}"]);
    if let HelperHop::Custom { program, .. } = &mut relative {
        *program = "relative/tool".into();
    }
    assert!(
        relative
            .validate()
            .unwrap_err()
            .to_string()
            .contains("absolute")
    );
    assert!(
        hop(&["--port", "1234"])
            .validate()
            .unwrap_err()
            .to_string()
            .contains("where to listen")
    );
    for value in [
        "{unknown}{local_port}",
        "{local_port}{unfinished",
        "{local_port}\0",
    ] {
        assert!(hop(&[value]).validate().is_err());
    }
    for timeout in [0, 301] {
        let mut invalid = hop(&["{local_port}"]);
        if let HelperHop::Custom {
            ready_timeout_seconds,
            ..
        } = &mut invalid
        {
            *ready_timeout_seconds = Some(timeout);
        }
        assert!(invalid.validate().is_err());
    }
}
#[tokio::test]
async fn literal_metacharacters_reach_program_as_one_argument() -> TestResult {
    let dir = tempfile::tempdir()?;
    let record = dir.path().join("args.json");
    let hop = hop(&[
        "--port",
        "{local_port}",
        "--record-args",
        record.to_str().ok_or("path")?,
        "a;b $(x) {local_port}",
        "{remote_host}:{remote_port}",
    ]);
    let spec = HelperSpec::new(
        hop,
        env!("CARGO_BIN_EXE_fake-cloud-hop").into(),
        "fake".into(),
    )
    .with_destination("db.internal".into(), 6432);
    let pool = Helpers::default();
    let helper = pool.acquire("literal", spec).await?;
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(record)?)?;
    assert_eq!(args[4], format!("a;b $(x) {}", helper.endpoint().port()));
    assert_eq!(args[5], "db.internal:6432");
    helper.close().await;
    pool.shutdown().await;
    Ok(())
}
