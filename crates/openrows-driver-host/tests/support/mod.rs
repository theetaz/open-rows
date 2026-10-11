// SPDX-License-Identifier: Apache-2.0
//! Disposable host/connection helpers with private short temporary endpoints.
use openrows_host_client::{
    CircuitBreaker, HostFailure, HostSession, HostState, HostSupervisor, LaunchOptions,
    MemoryBudget,
};
use openrows_host_protocol::{
    ArrowDecoder, Control, OwnedConnect, ProtocolError, Reply, WireEndpoint, WireFile, WireSummary,
    WireTls,
};
use std::{path::PathBuf, time::Duration};
#[cfg(target_os = "macos")]
mod image;

pub fn root() -> PathBuf {
    if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    }
}
/// Serialize the parent budget within libtest while keeping nextest processes independent.
pub async fn host_test() -> Result<tokio::sync::MutexGuard<'static, ()>, HostFailure> {
    static HOST_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    diagnostics();
    let permit = HOST_TEST.lock().await;
    prepare_image().await?;
    Ok(permit)
}
fn diagnostics() {
    static DIAGNOSTICS: std::sync::Once = std::sync::Once::new();
    DIAGNOSTICS.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_ansi(false)
            .without_time()
            .with_target(false)
            .with_writer(std::io::stderr)
            .try_init();
    });
}
/// Prepare macOS debug startup separately from the protocol deadline.
pub fn prepare_image() -> impl Future<Output = Result<(), HostFailure>> {
    #[cfg(target_os = "macos")]
    {
        image::prepare()
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::future::ready(Ok(()))
    }
}
pub fn options(memory: MemoryBudget) -> LaunchOptions {
    LaunchOptions {
        driver: openrows_host_client::HostDriverId::SqliteTest,
        executable: PathBuf::from(env!("CARGO_BIN_EXE_host-test-fixture")),
        endpoint_root: root(),
        memory,
        circuit: CircuitBreaker::default(),
    }
}
pub fn config() -> OwnedConnect {
    OwnedConnect {
        endpoint: WireEndpoint::File {
            source: WireFile::Memory {
                name: "test-only".into(),
            },
            attached: vec![],
        },
        database: "main".into(),
        user: String::new(),
        password: None,
        application: "test-only".into(),
        ssl_mode: 0,
        tls: WireTls::default(),
        timeouts_ms: [5000, 10000, 10000, 15000],
        startup: vec![],
        options: vec![],
    }
}
pub async fn host(memory: MemoryBudget) -> Result<HostSession, HostFailure> {
    let mut session = HostSupervisor::new(options(memory)).spawn().await?;
    session.send(1, &Control::Connect(config()))?;
    assert!(matches!(
        terminal(&mut session, 1).await?,
        Control::Reply(Reply::Connected(_))
    ));
    Ok(session)
}
pub async fn receive(
    session: &mut HostSession,
) -> Result<(openrows_host_protocol::Header, Control), HostFailure> {
    let frame = tokio::time::timeout(Duration::from_secs(7), session.recv())
        .await
        .map_err(|_| HostFailure::Unresponsive)??;
    let dto = frame.decode_control().map_err(HostFailure::from)?;
    Ok((frame.header, dto))
}
pub async fn terminal(session: &mut HostSession, request: u64) -> Result<Control, HostFailure> {
    loop {
        let (header, dto) = receive(session).await?;
        if header.request == request
            && matches!(
                dto,
                Control::Reply(_) | Control::Error(_) | Control::ConnectError { .. }
            )
        {
            return Ok(dto);
        }
    }
}
pub async fn execute(
    session: &mut HostSession,
    request: u64,
    sql: &str,
) -> Result<WireSummary, HostFailure> {
    session.send(request, &Control::Execute { sql: sql.into() })?;
    let mut decoder = ArrowDecoder::new();
    loop {
        let (_, dto) = receive(session).await?;
        match dto {
            Control::ResultBegin => decoder = ArrowDecoder::new(),
            Control::BatchReady(grant) => {
                session.send(0, &Control::Grant(grant))?;
                let bytes = data(session, grant.encoded as usize).await?;
                decoder.admit(&bytes, grant).map_err(HostFailure::from)?;
                session.send(0, &Control::Consumed(grant.key))?;
            }
            Control::ResultEnd { .. } => {
                decoder
                    .finish(&data(session, 8).await?)
                    .map_err(HostFailure::from)?;
            }
            Control::Reply(Reply::Executed(summary)) => return Ok(summary),
            Control::Error(_) => return Err(HostFailure::Protocol),
            _ => {}
        }
    }
}
pub async fn data(session: &mut HostSession, size: usize) -> Result<Vec<u8>, HostFailure> {
    let mut bytes = Vec::with_capacity(size);
    while bytes.len() < size {
        let frame = tokio::time::timeout(Duration::from_secs(5), session.read_data())
            .await
            .map_err(|_| HostFailure::Unresponsive)??;
        bytes.extend_from_slice(&frame.body);
    }
    if bytes.len() != size {
        return Err(ProtocolError::Invalid.into());
    }
    Ok(bytes)
}
pub async fn failed(session: &HostSession) -> HostFailure {
    let mut lifecycle = session.subscribe();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let state = *lifecycle.borrow_and_update();
            if let HostState::Failed(error) = state {
                return error;
            }
            lifecycle.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}
pub fn assert_reaped(pid: u32, directory: &std::path::Path) {
    assert!(
        !directory.exists(),
        "endpoint directory survived child {pid}"
    );
    #[cfg(windows)]
    assert!(
        !openrows_host_client::is_process_alive(pid).unwrap(),
        "child remains alive"
    );
    #[cfg(unix)]
    {
        let pid = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()).unwrap();
        assert_eq!(
            rustix::process::test_kill_process(pid),
            Err(rustix::io::Errno::SRCH),
            "child remains alive or unreaped"
        );
    }
}
