// SPDX-License-Identifier: Apache-2.0
//! `PRELOGIN` (MS-TDS 2.2.6.5): the client offers an encryption level and
//! the server answers with its version and the level both will use.

use openrows_driver_api::DriverError;

const VERSION: u8 = 0x00;
const ENCRYPTION: u8 = 0x01;
const INSTANCE: u8 = 0x02;
const THREAD_ID: u8 = 0x03;
const MARS: u8 = 0x04;
const TERMINATOR: u8 = 0xFF;

/// The encryption byte of `PRELOGIN`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Encryption {
    /// Encrypt the login packet only.
    Off = 0,
    /// Encrypt everything.
    On = 1,
    /// No encryption at all.
    NotSupported = 2,
    /// Encryption is required (a server answer).
    Required = 3,
}

impl Encryption {
    fn parse(byte: u8) -> Result<Self, DriverError> {
        // Bit 0x20 (client certificate authentication) does not change the level.
        match byte & 0x0F {
            0 => Ok(Self::Off),
            1 => Ok(Self::On),
            2 => Ok(Self::NotSupported),
            3 => Ok(Self::Required),
            other => Err(DriverError::Protocol(format!(
                "unknown PRELOGIN encryption value {other}"
            ))),
        }
    }
}

/// The server's answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreloginReply {
    /// Server version `(major, minor, build)`, e.g. `(16, 0, 4295)`.
    pub(crate) version: (u8, u8, u16),
    /// The encryption level the server chose.
    pub(crate) encryption: Encryption,
}

/// A `PRELOGIN` request offering `encryption`.
pub(crate) fn request(encryption: Encryption) -> Vec<u8> {
    let options: [(u8, Vec<u8>); 5] = [
        // Client version 1.0.0.0, sub-build 0.
        (VERSION, vec![1, 0, 0, 0, 0, 0]),
        (ENCRYPTION, vec![encryption as u8]),
        // The default instance: an empty, NUL-terminated name.
        (INSTANCE, vec![0]),
        (THREAD_ID, vec![0, 0, 0, 0]),
        // Multiple active result sets stay off.
        (MARS, vec![0]),
    ];
    let table_len = options.len() * 5 + 1;
    let mut table = Vec::with_capacity(table_len);
    let mut data = Vec::new();
    for (token, value) in &options {
        // The whole request is a few dozen bytes, so offsets fit in a u16.
        let offset = u16::try_from(table_len + data.len()).unwrap_or(u16::MAX);
        let len = u16::try_from(value.len()).unwrap_or(u16::MAX);
        table.push(*token);
        table.extend_from_slice(&offset.to_be_bytes());
        table.extend_from_slice(&len.to_be_bytes());
        data.extend_from_slice(value);
    }
    table.push(TERMINATOR);
    table.extend(data);
    table
}

/// Parse the server's `PRELOGIN` reply.
///
/// # Errors
/// `Protocol` when the option table or a value is malformed or missing.
pub(crate) fn parse_reply(payload: &[u8]) -> Result<PreloginReply, DriverError> {
    let malformed = || DriverError::Protocol("malformed PRELOGIN reply".to_owned());
    let mut version = None;
    let mut encryption = None;
    let mut at = 0;
    loop {
        let token = *payload.get(at).ok_or_else(malformed)?;
        if token == TERMINATOR {
            break;
        }
        let field = payload.get(at + 1..at + 5).ok_or_else(malformed)?;
        let offset = usize::from(u16::from_be_bytes([field[0], field[1]]));
        let len = usize::from(u16::from_be_bytes([field[2], field[3]]));
        let value = payload.get(offset..offset + len).ok_or_else(malformed)?;
        match (token, value) {
            (VERSION, [major, minor, high, low, ..]) => {
                version = Some((*major, *minor, u16::from_be_bytes([*high, *low])));
            }
            (ENCRYPTION, [byte, ..]) => encryption = Some(Encryption::parse(*byte)?),
            _ => {}
        }
        at += 5;
    }
    Ok(PreloginReply {
        version: version.ok_or_else(malformed)?,
        encryption: encryption.ok_or_else(malformed)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_carry_the_offered_level_and_parse_back() -> Result<(), DriverError> {
        let bytes = request(Encryption::On);
        assert_eq!(bytes[0], VERSION);
        assert_eq!(bytes[25], TERMINATOR);
        let reply = parse_reply(&bytes)?;
        assert_eq!(reply.encryption, Encryption::On);
        assert_eq!(reply.version, (1, 0, 0));
        Ok(())
    }

    #[test]
    fn server_replies_name_version_and_level() -> Result<(), DriverError> {
        let mut reply = vec![VERSION, 0, 11, 0, 6, ENCRYPTION, 0, 17, 0, 1, TERMINATOR];
        reply.extend_from_slice(&[16, 0, 0x10, 0xC7, 0, 0, 3]);
        let parsed = parse_reply(&reply)?;
        assert_eq!(parsed.version, (16, 0, 4295));
        assert_eq!(parsed.encryption, Encryption::Required);
        assert!(parse_reply(&[VERSION, 0, 99, 0, 6, TERMINATOR]).is_err());
        assert!(parse_reply(&[TERMINATOR]).is_err());
        Ok(())
    }
}
