// SPDX-License-Identifier: Apache-2.0
//! Buffered message I/O over a plain TCP stream (TLS arrives with M1-06).
use bytes::BytesMut;
use openrows_driver_api::DriverError;
use postgres_protocol::message::backend::{ErrorResponseBody, Message};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Socket read granularity; one read usually carries many `DataRow`s.
const READ_CHUNK: usize = 64 * 1024;

pub(crate) struct Wire {
    stream: TcpStream,
    read: BytesMut,
    write: BytesMut,
}

impl Wire {
    pub(crate) async fn connect(host: &str, port: u16) -> Result<Self, DriverError> {
        let stream = TcpStream::connect((host, port)).await?;
        stream.set_nodelay(true)?;
        Ok(Self {
            stream,
            read: BytesMut::with_capacity(READ_CHUNK),
            write: BytesMut::with_capacity(1024),
        })
    }

    /// Outgoing buffer for `postgres_protocol::message::frontend` encoders.
    pub(crate) fn out(&mut self) -> &mut BytesMut {
        &mut self.write
    }

    pub(crate) async fn flush(&mut self) -> Result<(), DriverError> {
        self.stream.write_all(&self.write).await?;
        self.write.clear();
        Ok(())
    }

    /// Next complete backend message; frames split across reads are buffered.
    pub(crate) async fn next(&mut self) -> Result<Message, DriverError> {
        loop {
            if let Some(message) = Message::parse(&mut self.read)? {
                return Ok(message);
            }
            if self.read.capacity() - self.read.len() < READ_CHUNK / 8 {
                self.read.reserve(READ_CHUNK);
            }
            if self.stream.read_buf(&mut self.read).await? == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into());
            }
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<(), DriverError> {
        self.flush().await?;
        self.stream.shutdown().await?;
        Ok(())
    }
}

/// Map an `ErrorResponse` to a typed error carrying its SQLSTATE.
pub(crate) fn server_error(body: &ErrorResponseBody) -> DriverError {
    use fallible_iterator::FallibleIterator;
    let (mut code, mut message) = (String::new(), String::new());
    let mut fields = body.fields();
    while let Ok(Some(field)) = fields.next() {
        match field.type_() {
            b'C' => code = String::from_utf8_lossy(field.value_bytes()).into_owned(),
            b'M' => message = String::from_utf8_lossy(field.value_bytes()).into_owned(),
            _ => {}
        }
    }
    DriverError::Server { code, message }
}

pub(crate) fn unexpected(context: &str) -> DriverError {
    DriverError::Protocol(format!("unexpected message during {context}"))
}
