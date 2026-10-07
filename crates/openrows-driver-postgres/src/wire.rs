// SPDX-License-Identifier: Apache-2.0
//! Buffered message I/O over a plain TCP stream (TLS arrives with M1-06).
use bytes::BytesMut;
use openrows_driver_api::{DriverError, NoticeLevel};
use postgres_protocol::message::backend::{
    ErrorFields, ErrorResponseBody, Message, NoticeResponseBody,
};
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

/// Code, message and non-localized severity fields of an error or notice.
#[derive(Default)]
struct Fields {
    code: String,
    message: String,
    severity: String,
}

fn fields(mut fields: ErrorFields<'_>) -> Fields {
    use fallible_iterator::FallibleIterator;
    let mut parsed = Fields::default();
    while let Ok(Some(field)) = fields.next() {
        let value = String::from_utf8_lossy(field.value_bytes()).into_owned();
        match field.type_() {
            b'C' => parsed.code = value,
            b'M' => parsed.message = value,
            b'V' => parsed.severity = value,
            _ => {}
        }
    }
    parsed
}

/// Map an `ErrorResponse` to a typed error carrying its SQLSTATE.
pub(crate) fn server_error(body: &ErrorResponseBody) -> DriverError {
    let Fields { code, message, .. } = fields(body.fields());
    DriverError::Server { code, message }
}

/// Level and message of a `NoticeResponse`; only `WARNING` maps to a warning.
pub(crate) fn notice(body: &NoticeResponseBody) -> (NoticeLevel, String) {
    let Fields {
        message, severity, ..
    } = fields(body.fields());
    let level = if severity == "WARNING" {
        NoticeLevel::Warning
    } else {
        NoticeLevel::Info
    };
    (level, message)
}

pub(crate) fn unexpected(context: &str) -> DriverError {
    DriverError::Protocol(format!("unexpected message during {context}"))
}
