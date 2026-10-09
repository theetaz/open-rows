// SPDX-License-Identifier: Apache-2.0
//! Deterministic fake helper lifecycle and process tree tests; no cloud accounts.
#![cfg(unix)]
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use openrows_net::{
    Dialer,
    helper::{Helper, HelperDialer, HelperError, HelperHop, HelperSpec, Helpers},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
type Result = std::result::Result<(), Box<dyn std::error::Error>>;
fn spec() -> HelperSpec {
    HelperSpec::new(
        HelperHop::Kubectl {
            context: "test".into(),
            namespace: "default".into(),
            resource: "svc/db".into(),
            remote_port: 5432,
            kubeconfig: None,
        },
        PathBuf::from(env!("CARGO_BIN_EXE_fake-cloud-hop")),
        "fake 1.0.0".into(),
    )
}
fn alive(pid: u32) -> bool {
    i32::try_from(pid).is_ok_and(|pid| kill(Pid::from_raw(pid), None).is_ok())
}
async fn until(check: impl Fn() -> bool) -> Result {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut poll = tokio::time::interval(Duration::from_millis(10));
        while !check() {
            poll.tick().await;
        }
    })
    .await?;
    Ok(())
}
fn crash(helper: &Helper) -> Result {
    kill(Pid::from_raw(i32::try_from(helper.pid())?), Signal::SIGKILL)?;
    Ok(())
}

#[tokio::test]
async fn two_sessions_share_and_last_release_kills_tree() -> Result {
    let folder = tempfile::tempdir()?;
    let child_file = folder.path().join("child-pid");
    let mut spec = spec();
    spec.environment.push((
        "OPENROWS_FAKE_CHILD_PID".into(),
        child_file.display().to_string(),
    ));
    let pool = Helpers::default();
    let first = pool.acquire("profile", spec.clone()).await?;
    let second = pool.acquire("profile", spec).await?;
    assert!(Arc::ptr_eq(&first, &second));
    let pid = first.pid();
    let child: u32 = std::fs::read_to_string(child_file)?.parse()?;
    let mut stream = HelperDialer::new(Arc::clone(&first)).dial().await?;
    drop(first);
    drop(second);
    assert!(alive(pid));
    stream.write_all(b"test").await?;
    let mut bytes = [0; 4];
    stream.read_exact(&mut bytes).await?;
    assert_eq!(&bytes, b"test");
    drop(stream);
    until(|| !alive(pid) && !alive(child)).await?;
    Ok(())
}
#[tokio::test]
async fn shutdown_hook_path_kills_even_old_profile_helpers() -> Result {
    let pool = Helpers::default();
    let first = pool.acquire("profile", spec()).await?;
    let mut edited = spec();
    edited.hop = HelperHop::Cloudflared {
        hostname: "db.example.invalid".into(),
    };
    let second = pool.acquire("profile", edited).await?;
    pool.shutdown().await;
    assert!(!alive(first.pid()) && !alive(second.pid()));
    assert!(pool.acquire("new", spec()).await.is_err());
    Ok(())
}
#[tokio::test]
async fn crash_restarts_once_then_reports_typed_scrubbed_stderr() -> Result {
    let pool = Helpers::default();
    let helper = pool.acquire("profile", spec()).await?;
    let first = helper.pid();
    crash(&helper)?;
    until(|| helper.pid() != first).await?;
    let dialer = HelperDialer::new(Arc::clone(&helper));
    let stream = dialer.dial().await?;
    drop(stream);
    crash(&helper)?;
    until(|| !alive(helper.pid())).await?;
    let mut error = None;
    for _ in 0..1000 {
        error = dialer.dial().await.err();
        if error
            .as_ref()
            .is_some_and(|error| error.to_string().contains("Exited again"))
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let error = error.ok_or("helper unexpectedly alive")?;
    let typed = error
        .get_ref()
        .and_then(|error| error.downcast_ref::<HelperError>());
    assert!(matches!(typed, Some(HelperError::Network { .. })));
    assert!(
        error.to_string().contains("Exited again")
            && error.to_string().contains("fake helper started")
    );
    assert!(!error.to_string().contains("token=x"));
    pool.shutdown().await;
    Ok(())
}
#[tokio::test]
async fn timeout_and_cancel_kill_children_and_missing_tool_has_hint() -> Result {
    let folder = tempfile::tempdir()?;
    let pid_file = folder.path().join("pid");
    let mut slow = spec();
    slow.timeout = Duration::from_secs(5);
    slow.environment = vec![
        ("OPENROWS_FAKE_NOT_READY".into(), "1".into()),
        ("OPENROWS_FAKE_PID".into(), pid_file.display().to_string()),
    ];
    let pool = Arc::new(Helpers::default());
    let error = pool
        .acquire("slow", slow.clone())
        .await
        .err()
        .ok_or("unexpected readiness")?;
    assert!(
        matches!(error, HelperError::Network { .. }) && error.to_string().contains("Timed out")
    );
    let pid: u32 = std::fs::read_to_string(&pid_file)?.parse()?;
    assert!(!alive(pid));
    std::fs::remove_file(&pid_file)?;
    assert!(error.to_string().contains("Version: fake"));
    assert!(error.to_string().contains("port-forward"));
    slow.timeout = Duration::from_secs(30);
    let pending_pool = Arc::clone(&pool);
    let pending = tokio::spawn(async move { pending_pool.acquire("cancel", slow).await });
    until(|| pid_file.exists()).await?;
    let pid: u32 = std::fs::read_to_string(pid_file)?.parse()?;
    pending.abort();
    let _ = pending.await;
    until(|| !alive(pid)).await?;
    let mut missing = spec();
    missing.path = folder.path().join("missing");
    let error = pool
        .acquire("missing", missing)
        .await
        .err()
        .ok_or("tool unexpectedly found")?;
    assert!(
        matches!(error, HelperError::NotFound { .. })
            && error.to_string().contains("Settings ▸ Tools")
    );
    Ok(())
}

#[tokio::test]
async fn shutdown_cancels_pending_readiness_and_kills_child() -> Result {
    let folder = tempfile::tempdir()?;
    let pid_file = folder.path().join("pending-pid");
    let mut slow = spec();
    slow.environment = vec![
        ("OPENROWS_FAKE_NOT_READY".into(), "1".into()),
        ("OPENROWS_FAKE_PID".into(), pid_file.display().to_string()),
    ];
    let pool = Arc::new(Helpers::default());
    let pending_pool = Arc::clone(&pool);
    let pending = tokio::spawn(async move { pending_pool.acquire("pending", slow).await });
    until(|| pid_file.exists()).await?;
    let pid: u32 = std::fs::read_to_string(pid_file)?.parse()?;
    tokio::time::timeout(Duration::from_secs(2), pool.shutdown()).await?;
    assert!(pending.await?.is_err());
    until(|| !alive(pid)).await?;
    Ok(())
}
