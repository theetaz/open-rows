// SPDX-License-Identifier: Apache-2.0
//! `LOGIN7` (MS-TDS 2.2.6.4) for SQL Server authentication: a 94-byte
//! fixed part with offsets into the UTF-16LE strings that follow. The
//! password is obfuscated as the protocol requires (nibble swap and
//! `XOR 0xA5`), which is not encryption: the packet itself travels inside
//! TLS unless encryption is disabled.

use super::cursor::utf16_bytes;

/// TDS 7.4, as the little-endian `TDSVersion` field.
const TDS_74: u32 = 0x7400_0004;
/// Length of the fixed part.
const FIXED_LEN: usize = 94;
/// `fUseDB`, `fDatabase` (a missing database fails the login) and
/// `fSetLang`.
const OPTION_FLAGS_1: u8 = 0xE0;
/// `fLanguage` (a missing language fails the login) and `fODBC`, which
/// makes the server apply the ODBC session defaults (`ANSI_DEFAULTS` on,
/// `TEXTSIZE` unlimited, `IMPLICIT_TRANSACTIONS` off).
const OPTION_FLAGS_2: u8 = 0x03;
/// `us_english`.
const CLIENT_LCID: u32 = 0x0409;
/// Library name reported to the server.
const LIBRARY: &str = "OpenRows";

/// What one login says about the client and the account.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Login<'a> {
    pub(crate) user: &'a str,
    pub(crate) password: &'a str,
    pub(crate) application: &'a str,
    /// The server name as the client addressed it.
    pub(crate) server: &'a str,
    /// Database to open; empty for the login's default database.
    pub(crate) database: &'a str,
    pub(crate) packet_size: u32,
}

/// The `LOGIN7` payload.
pub(crate) fn encode(login: &Login<'_>) -> Vec<u8> {
    let strings: [Vec<u8>; 9] = [
        utf16_bytes(LIBRARY),
        utf16_bytes(login.user),
        obfuscate(&utf16_bytes(login.password)),
        utf16_bytes(login.application),
        utf16_bytes(login.server),
        Vec::new(),
        utf16_bytes(LIBRARY),
        Vec::new(),
        utf16_bytes(login.database),
    ];
    let mut fixed = Vec::with_capacity(FIXED_LEN);
    let total = FIXED_LEN + strings.iter().map(Vec::len).sum::<usize>();
    push_u32(&mut fixed, total);
    fixed.extend_from_slice(&TDS_74.to_le_bytes());
    fixed.extend_from_slice(&login.packet_size.to_le_bytes());
    fixed.extend_from_slice(&[0, 0, 0, 7]);
    fixed.extend_from_slice(&std::process::id().to_le_bytes());
    fixed.extend_from_slice(&[0; 4]);
    fixed.extend_from_slice(&[OPTION_FLAGS_1, OPTION_FLAGS_2, 0, 0]);
    fixed.extend_from_slice(&[0; 4]);
    fixed.extend_from_slice(&CLIENT_LCID.to_le_bytes());
    let mut offset = FIXED_LEN;
    for (index, value) in strings.iter().enumerate() {
        push_u16(&mut fixed, offset);
        push_u16(&mut fixed, value.len() / 2);
        offset += value.len();
        if index == 8 {
            // ClientID (a MAC address) follows the database name.
            fixed.extend_from_slice(&[0; 6]);
        }
    }
    // SSPI, attached database file and change-password: unused.
    for _ in 0..3 {
        push_u16(&mut fixed, offset);
        push_u16(&mut fixed, 0);
    }
    fixed.extend_from_slice(&[0; 4]);
    fixed.extend(strings.into_iter().flatten());
    fixed
}

/// Swap the nibbles of each byte, then XOR with `0xA5`.
fn obfuscate(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .map(|byte| byte.rotate_left(4) ^ 0xA5)
        .collect()
}

fn push_u16(out: &mut Vec<u8>, value: usize) {
    // Every login field is far below 64 KiB.
    out.extend_from_slice(&u16::try_from(value).unwrap_or(u16::MAX).to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: usize) {
    out.extend_from_slice(&u32::try_from(value).unwrap_or(u32::MAX).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(packet: &[u8], index: usize) -> (usize, usize) {
        let at = 36 + index * 4;
        let at = if index > 8 { at + 6 } else { at };
        let read =
            |start: usize| usize::from(u16::from_le_bytes([packet[start], packet[start + 1]]));
        (read(at), read(at + 2))
    }

    #[test]
    fn strings_are_located_by_offset_and_length() {
        let login = Login {
            user: "sa",
            password: "pw",
            application: "OpenRows",
            server: "db.local",
            database: "shop",
            packet_size: 4096,
        };
        let packet = encode(&login);
        assert_eq!(&packet[4..8], &[4, 0, 0, 0x74]);
        assert_eq!(
            packet.len(),
            usize::try_from(u32::from_le_bytes([
                packet[0], packet[1], packet[2], packet[3]
            ]))
            .unwrap_or(0)
        );
        let (offset, chars) = field(&packet, 1);
        assert_eq!(&packet[offset..offset + chars * 2], &utf16_bytes("sa")[..]);
        let (offset, chars) = field(&packet, 8);
        assert_eq!(
            &packet[offset..offset + chars * 2],
            &utf16_bytes("shop")[..]
        );
        let (offset, chars) = field(&packet, 2);
        assert_eq!(chars, 2);
        assert_eq!(
            &packet[offset..offset + 4],
            &obfuscate(&utf16_bytes("pw"))[..]
        );
        assert_ne!(&packet[offset..offset + 4], &utf16_bytes("pw")[..]);
    }

    #[test]
    fn obfuscation_matches_the_protocol_example() {
        // 'a' (0x61) swaps to 0x16, XOR 0xA5 gives 0xB3; the high byte 0x00 gives 0xA5.
        assert_eq!(obfuscate(&[0x61, 0x00]), [0xB3, 0xA5]);
    }
}
