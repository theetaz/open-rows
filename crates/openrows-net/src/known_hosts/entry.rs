// SPDX-License-Identifier: Apache-2.0
//! One `known_hosts` line: optional marker, host patterns (plain or hashed), key.

use data_encoding::BASE64;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::PublicKey;
use sha1::Sha1;

use crate::config::pattern_list_matches;

/// Line marker (`@revoked`, `@cert-authority`) or none.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Marker {
    None,
    Revoked,
    CertAuthority,
}

/// A parsed `known_hosts` line.
#[derive(Debug)]
pub(crate) struct Entry<'a> {
    pub(crate) marker: Marker,
    pub(crate) hosts: &'a str,
    pub(crate) key: PublicKey,
}

/// Parse one line; comments, blank, unknown-marker and malformed lines are `None`
/// (skipped, as OpenSSH does, so one bad line cannot hide the others).
pub(crate) fn parse_entry(line: &str) -> Option<Entry<'_>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut fields = line.split_whitespace();
    let mut hosts = fields.next()?;
    let marker = match hosts {
        "@revoked" => Marker::Revoked,
        "@cert-authority" => Marker::CertAuthority,
        other if other.starts_with('@') => return None,
        _ => Marker::None,
    };
    if marker != Marker::None {
        hosts = fields.next()?;
    }
    let algorithm = fields.next()?;
    let data = fields.next()?;
    let key = PublicKey::from_openssh(&format!("{algorithm} {data}")).ok()?;
    Some(Entry { marker, hosts, key })
}

/// Whether the host field matches `host_port` (`host` or `[host]:port`).
pub(crate) fn hosts_match(hosts: &str, host_port: &str) -> bool {
    if let Some(hashed) = hosts.strip_prefix("|1|") {
        return hashed_match(hashed, host_port);
    }
    let patterns: Vec<&str> = hosts.split(',').collect();
    pattern_list_matches(&patterns, host_port)
}

/// `|1|base64(salt)|base64(HMAC-SHA1(salt, host))`, as written by `ssh-keygen -H`.
fn hashed_match(hashed: &str, host_port: &str) -> bool {
    let Some((salt, hash)) = hashed.split_once('|') else {
        return false;
    };
    let (Ok(salt), Ok(hash)) = (
        BASE64.decode(salt.as_bytes()),
        BASE64.decode(hash.as_bytes()),
    ) else {
        return false;
    };
    let Ok(mac) = Hmac::<Sha1>::new_from_slice(&salt) else {
        return false;
    };
    mac.chain_update(host_port.as_bytes())
        .verify_slice(&hash)
        .is_ok()
}

#[cfg(test)]
pub(crate) fn hash_host(salt: &[u8], host_port: &str) -> String {
    let mac = Hmac::<Sha1>::new_from_slice(salt).expect("HMAC accepts any key length");
    let digest = mac
        .chain_update(host_port.as_bytes())
        .finalize()
        .into_bytes();
    format!("|1|{}|{}", BASE64.encode(salt), BASE64.encode(&digest))
}
