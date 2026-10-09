// SPDX-License-Identifier: Apache-2.0
//! `ENVCHANGE` (MS-TDS 2.2.7.9): the server reports a new database, packet
//! size or collation, and every transaction it begins or ends. The
//! transaction descriptor of `BEGIN` is what requests must carry while the
//! transaction is open, and its presence is the session's transaction
//! state.

use super::cursor::{Cursor, Parsed};
use super::token::whole;

/// What changed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum EnvChange {
    Database(String),
    PacketSize(usize),
    /// The database's default collation, as five wire bytes.
    Collation(Vec<u8>),
    /// A transaction began; requests carry this descriptor.
    BeginTransaction(u64),
    /// The transaction committed, rolled back or otherwise ended.
    EndTransaction,
    /// Azure SQL redirects the client to another host and port.
    Routing {
        host: String,
        port: u16,
    },
    Other,
}

pub(crate) fn env_change(cursor: &mut Cursor<'_>) -> Parsed<EnvChange> {
    let len = cursor.u16()?;
    let mut body = Cursor::new(cursor.take(usize::from(len))?);
    let parsed = (|| {
        let kind = body.u8()?;
        Ok(match kind {
            1 => EnvChange::Database(body.b_varchar()?),
            4 => {
                let size = body.b_varchar()?;
                size.parse().map_or(EnvChange::Other, EnvChange::PacketSize)
            }
            7 => EnvChange::Collation(body.b_varbyte()?.to_vec()),
            8 => {
                let descriptor = body.b_varbyte()?;
                let bytes: [u8; 8] = descriptor.try_into().unwrap_or_default();
                EnvChange::BeginTransaction(u64::from_le_bytes(bytes))
            }
            9 | 10 | 12 | 17 => EnvChange::EndTransaction,
            20 => routing(&mut body)?,
            _ => EnvChange::Other,
        })
    })();
    whole(parsed, "ENVCHANGE")
}

/// Routing data: length, protocol (0 is TCP), port and server name.
fn routing(body: &mut Cursor<'_>) -> Parsed<EnvChange> {
    body.u16()?;
    body.u8()?;
    let port = body.u16()?;
    let host = body.us_varchar()?;
    Ok(EnvChange::Routing { host, port })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tds::cursor::utf16_bytes;

    fn parse(kind: u8, data: &[u8]) -> Option<EnvChange> {
        let mut bytes = u16::try_from(data.len() + 1)
            .unwrap_or(0)
            .to_le_bytes()
            .to_vec();
        bytes.push(kind);
        bytes.extend_from_slice(data);
        env_change(&mut Cursor::new(&bytes)).ok()
    }

    #[test]
    fn changes_are_typed() {
        let mut database = vec![4];
        database.extend(utf16_bytes("shop"));
        database.push(0);
        assert_eq!(
            parse(1, &database),
            Some(EnvChange::Database("shop".to_owned()))
        );
        let begin = [8, 1, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(parse(8, &begin), Some(EnvChange::BeginTransaction(1)));
        assert_eq!(
            parse(10, &[0, 8, 1, 0, 0, 0, 0, 0, 0, 0]),
            Some(EnvChange::EndTransaction)
        );
        let mut size = vec![4];
        size.extend(utf16_bytes("8000"));
        size.push(0);
        assert_eq!(parse(4, &size), Some(EnvChange::PacketSize(8000)));
        assert_eq!(parse(99, &[1, 2]), Some(EnvChange::Other));
        assert!(parse(1, &[9]).is_none());
    }
}
