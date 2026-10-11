// SPDX-License-Identifier: Apache-2.0
//! Multiple results and stop barriers through the raw hosted protocol.
#[cfg(target_os = "macos")]
#[path = "support/duckdb_image.rs"]
mod image;
#[path = "support/duckdb_launch.rs"]
mod launch;
use launch::options;
use openrows_host_client::{HostSession, HostSupervisor, MemoryBudget};
use openrows_host_protocol::{Control, Grant, OwnedConnect, Reply};
use std::time::Duration;
type Error = Box<dyn std::error::Error>;
async fn receive(host: &mut HostSession) -> Result<Control, Error> {
    Ok(tokio::time::timeout(Duration::from_secs(5), host.recv())
        .await??
        .decode_control()?)
}
async fn open(name: &str) -> Result<HostSession, Error> {
    #[cfg(target_os = "macos")]
    image::prepare().await?;
    let mut host = HostSupervisor::new(options(MemoryBudget::default()))
        .spawn()
        .await?;
    host.send(1, &Control::Connect(config(name)))?;
    loop {
        match receive(&mut host).await? {
            Control::Reply(Reply::Connected(_)) => return Ok(host),
            Control::Accepted | Control::Heartbeat => {}
            other => return Err(format!("unexpected connect: {other:?}").into()),
        }
    }
}
async fn drain(host: &mut HostSession, grant: Grant) -> Result<(), Error> {
    host.send(0, &Control::Grant(grant))?;
    let mut size = 0;
    while size < grant.encoded as usize {
        size += tokio::time::timeout(Duration::from_secs(5), host.read_data())
            .await??
            .body
            .len();
    }
    assert_eq!(size, grant.encoded as usize);
    Ok(())
}
#[tokio::test]
async fn multiple_results_have_ordered_barriers() -> Result<(), Error> {
    let mut host = open("protocol-test").await?;
    host.send(
        2,
        &Control::Execute {
            sql: "SELECT 1; SELECT 2".into(),
        },
    )?;
    let mut results = 0;
    loop {
        match receive(&mut host).await? {
            Control::ResultBegin => results += 1,
            Control::BatchReady(grant) => {
                drain(&mut host, grant).await?;
                host.send(0, &Control::Consumed(grant.key))?;
            }
            Control::ResultEnd { .. } => {
                let eos = tokio::time::timeout(Duration::from_secs(5), host.read_data()).await??;
                assert_eq!(eos.body.as_slice(), [255, 255, 255, 255, 0, 0, 0, 0]);
            }
            Control::Reply(Reply::Executed(summary)) => {
                assert_eq!(summary.rows, 2);
                break;
            }
            Control::Error(error) => return Err(format!("{error:?}").into()),
            _ => {}
        }
    }
    assert_eq!(results, 2);
    host.close().await?;
    Ok(())
}
#[tokio::test]
async fn stopped_native_result_has_a_terminal_barrier() -> Result<(), Error> {
    let mut host = open("stop-protocol-test").await?;
    host.send(
        2,
        &Control::Execute {
            sql: "SELECT i FROM range(10000) t(i) ORDER BY i".into(),
        },
    )?;
    loop {
        match receive(&mut host).await? {
            Control::BatchReady(grant) => {
                drain(&mut host, grant).await?;
                host.send(0, &Control::Stop(grant.key))?;
                host.send(0, &Control::Consumed(grant.key))?;
            }
            Control::ResultEnd { sequence } => assert_eq!(sequence, 1),
            Control::Reply(Reply::Executed(summary)) => {
                assert_eq!(summary.rows, 200);
                break;
            }
            Control::Error(error) => return Err(format!("{error:?}").into()),
            _ => {}
        }
    }
    host.close().await?;
    Ok(())
}

fn config(name: &str) -> OwnedConnect {
    use openrows_host_protocol::{WireEndpoint, WireFile, WireTls};
    OwnedConnect {
        endpoint: WireEndpoint::File {
            source: WireFile::Memory { name: name.into() },
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
