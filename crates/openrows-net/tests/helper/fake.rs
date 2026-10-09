// SPDX-License-Identifier: Apache-2.0
//! Test-only binary simulating cloud helper flags and a TCP forwarding process tree.
use std::{io::Write as _, net::SocketAddr};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args
        .iter()
        .any(|arg| arg == "--version" || arg == "version")
    {
        writeln!(std::io::stdout(), "fake-cloud-hop 1.0.0")?;
        return Ok(());
    }
    if args.first().is_some_and(|arg| arg == "--descendant") {
        std::future::pending::<()>().await;
        return Ok(());
    }
    if let Ok(file) = std::env::var("OPENROWS_FAKE_PID") {
        write_pid(&file, std::process::id())?;
    }
    let _descendant = if let Ok(file) = std::env::var("OPENROWS_FAKE_CHILD_PID") {
        let child = tokio::process::Command::new(std::env::current_exe()?)
            .arg("--descendant")
            .spawn()?;
        write_pid(&file, child.id().ok_or("missing child PID")?)?;
        Some(child)
    } else {
        None
    };
    writeln!(std::io::stderr(), "fake helper started")?;
    writeln!(std::io::stderr(), "token=x")?;
    if std::env::var_os("OPENROWS_FAKE_NOT_READY").is_some() {
        std::future::pending::<()>().await;
    }
    let address = address(&args)?;
    let listener = TcpListener::bind(address).await?;
    loop {
        let (stream, _) = listener.accept().await?;
        let target = std::env::var("OPENROWS_FAKE_TARGET").ok();
        tokio::spawn(async move {
            let _ = forward(stream, target).await;
        });
    }
}
fn address(args: &[String]) -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let flag = |name: &str| {
        args.windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].as_str())
    };
    let port = if let Some(json) = flag("--parameters") {
        let parameters: serde_json::Value = serde_json::from_str(json)?;
        parameters["localPortNumber"][0]
            .as_str()
            .ok_or("missing local port")?
            .parse()?
    } else if let Some(port) = flag("--port") {
        port.parse()?
    } else if let Some(url) = flag("--url") {
        return Ok(url.parse()?);
    } else {
        args.last()
            .ok_or("missing port")?
            .split_once(':')
            .ok_or("missing port mapping")?
            .0
            .parse()?
    };
    Ok(SocketAddr::from(([127, 0, 0, 1], port)))
}
async fn forward(mut stream: TcpStream, target: Option<String>) -> std::io::Result<()> {
    if let Some(target) = target {
        let mut remote = TcpStream::connect(target).await?;
        tokio::io::copy_bidirectional(&mut stream, &mut remote).await?;
    } else {
        let mut bytes = [0; 1024];
        loop {
            let count = stream.read(&mut bytes).await?;
            if count == 0 {
                break;
            }
            stream.write_all(&bytes[..count]).await?;
        }
    }
    Ok(())
}

fn write_pid(path: &str, pid: u32) -> std::io::Result<()> {
    let pending = format!("{path}.pending");
    std::fs::write(&pending, pid.to_string())?;
    std::fs::rename(pending, path)
}
