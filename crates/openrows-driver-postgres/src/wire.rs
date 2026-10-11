// SPDX-License-Identifier: Apache-2.0
//! Buffered message I/O over a TCP, Unix, TLS or routed stream.
use bytes::BytesMut;
use openrows_driver_api::{DriverError, NoticeLevel};
use postgres_protocol::message::backend::{
    ErrorFields, ErrorResponseBody, Message, NoticeResponseBody,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Socket read granularity; one read usually carries many `DataRow`s.
const READ_CHUNK: usize = 64 * 1024;

/// Byte stream under the protocol; boxed so TCP, Unix, TLS and routed streams share one type.
pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

pub(crate) struct Wire {
    message_limit: usize,
    invalidated: bool,
    stream: Box<dyn Io>,
    read: BytesMut,
    write: BytesMut,
    /// Transaction status of the last `ReadyForQuery` (`I` idle, `T` in a
    /// transaction, `E` in a failed transaction).
    status: u8,
}

impl Wire {
    pub(crate) fn new(stream: Box<dyn Io>) -> Self {
        Self {
            message_limit: usize::MAX,
            invalidated: false,
            stream,
            read: BytesMut::with_capacity(READ_CHUNK),
            write: BytesMut::with_capacity(1024),
            status: b'I',
        }
    }

    /// True when the server reported an open (or failed) transaction at the
    /// last `ReadyForQuery`.
    pub(crate) fn in_transaction(&self) -> bool {
        self.status != b'I'
    }

    /// Outgoing buffer for `postgres_protocol::message::frontend` encoders.
    pub(crate) fn out(&mut self) -> &mut BytesMut {
        &mut self.write
    }

    pub(crate) async fn flush(&mut self) -> Result<(), DriverError> {
        self.usable()?;
        self.stream.write_all(&self.write).await?;
        self.write.clear();
        Ok(())
    }

    /// Next complete backend message; frames split across reads are buffered.
    pub(crate) async fn next(&mut self) -> Result<Message, DriverError> {
        self.usable()?;
        loop {
            self.admit_frame()?;
            if let Some(message) = Message::parse(&mut self.read)? {
                self.observe(&message);
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

    /// Bound dedicated monitor frames before allocating their payloads.
    pub(crate) fn limit_messages(&mut self, bytes: usize) {
        self.message_limit = bytes;
    }

    /// Prevent any further wire use after an undrained monitor failure.
    pub(crate) fn invalidate(&mut self) {
        self.invalidated = true;
        self.write.clear();
    }

    fn usable(&self) -> Result<(), DriverError> {
        if self.invalidated {
            Err(DriverError::Protocol("monitor session discarded".into()))
        } else {
            Ok(())
        }
    }

    fn admit_frame(&self) -> Result<(), DriverError> {
        if self.read.len() < 5 {
            return Ok(());
        }
        let size =
            u32::from_be_bytes([self.read[1], self.read[2], self.read[3], self.read[4]]) as usize;
        if size > self.message_limit {
            return Err(DriverError::Protocol("monitor frame limit exceeded".into()));
        }
        Ok(())
    }

    /// Remember the transaction status every `ReadyForQuery` reports.
    fn observe(&mut self, message: &Message) {
        if let Message::ReadyForQuery(body) = message {
            self.status = body.status();
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<(), DriverError> {
        self.flush().await?;
        self.stream.shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn discarded_monitor_protocol_cannot_read_or_write_again() {
        let (stream, _peer) = tokio::io::duplex(64);
        let mut wire = Wire::new(Box::new(stream));
        wire.invalidate();
        assert!(matches!(wire.flush().await, Err(DriverError::Protocol(_))));
        assert!(matches!(wire.next().await, Err(DriverError::Protocol(_))));
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
