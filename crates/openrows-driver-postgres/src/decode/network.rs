// SPDX-License-Identifier: Apache-2.0
//! `inet`, `cidr`, `macaddr` and `macaddr8` binary values as `PostgreSQL` text.
use super::scalar::CellError;
use std::fmt::Write;
use std::net::{Ipv4Addr, Ipv6Addr};

/// `PGSQL_AF_INET` and `PGSQL_AF_INET6` from `utils/inet.h`.
const FAMILY_V4: u8 = 2;
const FAMILY_V6: u8 = 3;

/// Family, prefix bits, `is_cidr`, address length, then the address bytes.
/// `inet` hides a full-length prefix; `cidr` always shows it.
pub(crate) fn write_inet(bytes: &[u8], out: &mut impl Write) -> Result<(), CellError> {
    let [family, bits, cidr, length, address @ ..] = bytes else {
        return Err(CellError::Truncated);
    };
    if usize::from(*length) != address.len() {
        return Err(CellError::Truncated);
    }
    let full = match (*family, address.len()) {
        (FAMILY_V4, 4) => {
            let octets: [u8; 4] = address.try_into().map_err(|_| CellError::Truncated)?;
            write!(out, "{}", Ipv4Addr::from(octets)).map_err(|_| CellError::Format)?;
            32
        }
        (FAMILY_V6, 16) => {
            let octets: [u8; 16] = address.try_into().map_err(|_| CellError::Truncated)?;
            write!(out, "{}", Ipv6Addr::from(octets)).map_err(|_| CellError::Format)?;
            128
        }
        _ => return Err(CellError::Truncated),
    };
    if *cidr != 0 || *bits != full {
        write!(out, "/{bits}").map_err(|_| CellError::Format)?;
    }
    Ok(())
}

/// Six or eight bytes as lowercase colon-separated hex.
pub(crate) fn write_macaddr(bytes: &[u8], out: &mut impl Write) -> Result<(), CellError> {
    if bytes.len() != 6 && bytes.len() != 8 {
        return Err(CellError::Length {
            expected: 6,
            actual: bytes.len(),
        });
    }
    for (index, byte) in bytes.iter().enumerate() {
        let separator = if index == 0 { "" } else { ":" };
        write!(out, "{separator}{byte:02x}").map_err(|_| CellError::Format)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inet(bytes: &[u8]) -> Result<String, CellError> {
        let mut out = String::new();
        write_inet(bytes, &mut out)?;
        Ok(out)
    }

    #[test]
    fn addresses_print_like_postgres() -> Result<(), CellError> {
        assert_eq!(inet(&[2, 32, 0, 4, 192, 168, 0, 1])?, "192.168.0.1");
        assert_eq!(inet(&[2, 24, 0, 4, 192, 168, 0, 1])?, "192.168.0.1/24");
        assert_eq!(inet(&[2, 16, 1, 4, 10, 1, 0, 0])?, "10.1.0.0/16");
        assert_eq!(inet(&[2, 32, 1, 4, 10, 1, 0, 1])?, "10.1.0.1/32");
        let mut v6 = vec![3, 128, 0, 16];
        v6.extend_from_slice(&Ipv6Addr::LOCALHOST.octets());
        assert_eq!(inet(&v6)?, "::1");
        v6[1] = 64;
        assert_eq!(inet(&v6)?, "::1/64");
        assert_eq!(inet(&[2, 32, 0, 5, 1, 2, 3, 4]), Err(CellError::Truncated));
        assert_eq!(inet(&[9]), Err(CellError::Truncated));
        Ok(())
    }

    #[test]
    fn mac_addresses_are_lowercase_hex() -> Result<(), CellError> {
        let mut out = String::new();
        write_macaddr(&[0x08, 0, 0x2b, 1, 2, 0xAB], &mut out)?;
        assert_eq!(out, "08:00:2b:01:02:ab");
        out.clear();
        write_macaddr(&[0x08, 0, 0x2b, 0xff, 0xfe, 1, 2, 3], &mut out)?;
        assert_eq!(out, "08:00:2b:ff:fe:01:02:03");
        assert!(write_macaddr(&[1, 2], &mut out).is_err());
        Ok(())
    }
}
