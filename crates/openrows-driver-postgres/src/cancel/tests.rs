// SPDX-License-Identifier: Apache-2.0
use super::*;
use tokio::net::{TcpListener, TcpStream};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

async fn endpoint() -> TestResult<(PgCancel, TcpListener)> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let target = Target::Tcp {
        host: "127.0.0.1".to_owned(),
        port: listener.local_addr()?.port(),
    };
    let key = BackendKey {
        process_id: 1,
        secret_key: 2,
    };
    Ok((PgCancel::new(target, key), listener))
}

async fn received(listener: TcpListener) -> TestResult<TcpStream> {
    let (mut stream, _) = listener.accept().await?;
    let mut packet = [0; 16];
    stream.read_exact(&mut packet).await?;
    assert_eq!(&packet[..8], &[0, 0, 0, 16, 4, 210, 22, 46]);
    Ok(stream)
}

#[tokio::test]
async fn idle_and_delayed_requests_never_open_a_cancel_connection() -> TestResult {
    let (cancel, listener) = endpoint().await?;
    let idle = cancel.cancel();
    let first = cancel.begin().await?;
    idle.await?;
    let finished = cancel.cancel();
    let next = cancel.cancel();
    first.finish().await?;
    finished.await?;
    let second = cancel.begin().await?;
    next.await?;
    second.finish().await?;
    let listener = listener.into_std()?;
    let accepted = listener.accept().map(drop).map_err(|error| error.kind());
    assert_eq!(accepted, Err(std::io::ErrorKind::WouldBlock));
    Ok(())
}

#[tokio::test]
async fn statement_completion_waits_for_server_eof() -> TestResult {
    let (cancel, listener) = endpoint().await?;
    let statement = cancel.begin().await?;
    let mut request = cancel.cancel();
    let server = tokio::spawn(received(listener));
    let mut stream = tokio::select! {
        result = &mut request => return Err(format!("cancel returned before EOF: {result:?}").into()),
        stream = server => stream??,
    };
    let mut finish = Box::pin(statement.finish());
    tokio::select! {
        biased;
        result = &mut finish => return Err(format!("statement escaped cancel fence: {result:?}").into()),
        () = tokio::task::yield_now() => {},
    }
    stream.shutdown().await?;
    let (cancelled, finished) = tokio::join!(request, finish);
    cancelled?;
    finished?;
    cancel.begin().await?.finish().await?;
    Ok(())
}

#[tokio::test]
async fn abandoned_sent_request_prevents_session_reuse() -> TestResult {
    let (cancel, listener) = endpoint().await?;
    let statement = cancel.begin().await?;
    let mut request = cancel.cancel();
    let server = tokio::spawn(received(listener));
    let _stream = tokio::select! {
        result = &mut request => return Err(format!("cancel returned before EOF: {result:?}").into()),
        stream = server => stream??,
    };
    drop(request);
    assert!(matches!(
        statement.finish().await,
        Err(DriverError::Protocol(_))
    ));
    assert!(matches!(
        cancel.begin().await,
        Err(DriverError::Protocol(_))
    ));
    Ok(())
}

#[tokio::test]
async fn cancel_socket_data_is_a_protocol_error_and_prevents_reuse() -> TestResult {
    let (cancel, listener) = endpoint().await?;
    let statement = cancel.begin().await?;
    let server = async {
        let mut stream = received(listener).await?;
        stream.write_all(b"x").await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let (result, response) = tokio::join!(cancel.cancel(), server);
    response?;
    assert!(matches!(result, Err(DriverError::Protocol(_))));
    assert!(matches!(
        statement.finish().await,
        Err(DriverError::Protocol(_))
    ));
    Ok(())
}

#[tokio::test]
async fn dropped_statement_can_be_cancelled_but_never_reused() -> TestResult {
    let (cancel, listener) = endpoint().await?;
    drop(cancel.begin().await?);
    let server = async {
        let mut stream = received(listener).await?;
        stream.shutdown().await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let (result, response) = tokio::join!(cancel.cancel(), server);
    response?;
    result?;
    assert!(matches!(
        cancel.begin().await,
        Err(DriverError::Protocol(_))
    ));
    Ok(())
}
