// SPDX-License-Identifier: Apache-2.0
use std::path::Path;

use proptest::prelude::*;
use russh::keys::PublicKey;

use super::{
    HostKeyStatus, KnownHosts, KnownHostsLine, check, entry::hash_host, fingerprint, host_port,
};

// Synthetic public keys generated once for these tests (public data only).
const KEY_A: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBPwVzIBaK7j6/qUUcMccb2UJgRMNoC5s5ZFAw+yTDkY";
const KEY_B: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIL6IzzsMUnE72ggKzADPJJUUWPRcLHkQ8uPiWuZq1pZg";
const KEY_C: &str = "ecdsa-sha2-nistp256 AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBA72xNqmMKbhzpHiQB1KfaosIpqfSVyhkengfZDkZnVi724maXa/MtBcUX6mJcj3wucVoIjaLQi3LiZQOHHjXw0=";
const FINGERPRINT_A: &str = "SHA256:9WgxvJuHfDGC4yDyruhIFXJ3F1OcaoMP0Ku/ppr7MkY";

fn key(text: &str) -> PublicKey {
    PublicKey::from_openssh(text).expect("fixture key")
}

fn status(text: &str, host: &str, presented: &str) -> HostKeyStatus {
    check(text, Path::new("/kh"), host, &key(presented))
}

fn line(line: usize) -> KnownHostsLine {
    KnownHostsLine {
        path: "/kh".into(),
        line,
    }
}

#[test]
fn fingerprint_matches_ssh_keygen() {
    assert_eq!(fingerprint(&key(KEY_A)), FINGERPRINT_A);
}

#[test]
fn host_port_uses_brackets_for_non_default_ports() {
    assert_eq!(host_port("Bastion.Example", 22), "bastion.example");
    assert_eq!(host_port("10.0.0.1", 2222), "[10.0.0.1]:2222");
}

#[test]
fn matching_key_is_trusted() {
    let text = format!("# comment\nother {KEY_B}\nbastion,10.0.0.1 {KEY_A}\n");
    assert_eq!(status(&text, "10.0.0.1", KEY_A), HostKeyStatus::Trusted);
}

#[test]
fn unknown_host_reports_fingerprint_and_other_algorithms() {
    let text = format!("bastion {KEY_C}\n");
    assert_eq!(
        status(&text, "bastion", KEY_A),
        HostKeyStatus::Unknown {
            fingerprint: FINGERPRINT_A.into(),
            other_algorithms: vec!["ecdsa-sha2-nistp256".into()],
        }
    );
}

#[test]
fn same_algorithm_different_key_is_changed() {
    let text = format!("bastion {KEY_C}\n[bastion]:2222 {KEY_B}\n");
    assert_eq!(
        status(&text, "[bastion]:2222", KEY_A),
        HostKeyStatus::Changed {
            fingerprint: FINGERPRINT_A.into(),
            at: line(2),
        }
    );
}

#[test]
fn hashed_entries_and_wildcards_match() {
    let hashed = hash_host(b"0123456789abcdefghij", "[db.internal]:2200");
    let text = format!("{hashed} {KEY_A}\n*.example,!evil.example {KEY_B}\n");
    assert_eq!(
        status(&text, "[db.internal]:2200", KEY_A),
        HostKeyStatus::Trusted
    );
    assert!(matches!(
        status(&text, "[other]:2200", KEY_A),
        HostKeyStatus::Unknown { .. }
    ));
    assert_eq!(status(&text, "api.example", KEY_B), HostKeyStatus::Trusted);
    assert!(matches!(
        status(&text, "evil.example", KEY_B),
        HostKeyStatus::Unknown { .. }
    ));
}

#[test]
fn revoked_key_wins_over_trusted_entry() {
    let text = format!("bastion {KEY_A}\n@revoked * {KEY_A}\n");
    assert_eq!(
        status(&text, "bastion", KEY_A),
        HostKeyStatus::Revoked {
            fingerprint: FINGERPRINT_A.into(),
            at: line(2),
        }
    );
}

#[test]
fn malformed_lines_and_cert_authorities_are_skipped() {
    let text = format!(
        "garbage\nbastion ssh-ed25519 !!!\n@cert-authority * {KEY_B}\n@future bastion {KEY_B}\nbastion {KEY_A}\n"
    );
    assert_eq!(status(&text, "bastion", KEY_A), HostKeyStatus::Trusted);
    assert!(matches!(
        status(&text, "bastion", KEY_B),
        HostKeyStatus::Changed { .. }
    ));
}

#[test]
fn store_learns_into_app_file_only() {
    let dir = tempfile::tempdir().expect("temp dir");
    let user_file = dir.path().join("user_known_hosts");
    std::fs::write(&user_file, format!("bastion {KEY_C}")).expect("write fixture");
    let app_file = dir.path().join("app/known_hosts");
    let store = KnownHosts::new(vec![user_file.clone()], app_file.clone());

    assert!(matches!(
        store.verify("bastion", 22, &key(KEY_A)),
        Ok(HostKeyStatus::Unknown { .. })
    ));
    store
        .remember("bastion", 2222, &key(KEY_A))
        .expect("remember");
    store
        .remember("bastion", 22, &key(KEY_A))
        .expect("remember");

    assert_eq!(
        store.verify("bastion", 22, &key(KEY_A)).expect("verify"),
        HostKeyStatus::Trusted
    );
    assert_eq!(
        store
            .known_algorithms("bastion", 22)
            .expect("algorithms")
            .len(),
        2
    );
    assert_eq!(
        std::fs::read_to_string(&user_file).expect("read"),
        format!("bastion {KEY_C}")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&app_file)
            .expect("metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn store_changed_in_any_file_is_reported_unless_trusted_elsewhere() {
    let dir = tempfile::tempdir().expect("temp dir");
    let user_file = dir.path().join("known_hosts");
    std::fs::write(&user_file, format!("bastion {KEY_B}\n")).expect("write fixture");
    let store = KnownHosts::new(vec![user_file], dir.path().join("app"));
    assert!(matches!(
        store.verify("bastion", 22, &key(KEY_A)),
        Ok(HostKeyStatus::Changed { .. })
    ));
    store
        .remember("bastion", 22, &key(KEY_A))
        .expect("remember");
    assert_eq!(
        store.verify("bastion", 22, &key(KEY_A)).expect("verify"),
        HostKeyStatus::Trusted
    );
}

proptest! {
    #[test]
    fn checker_never_panics(text in "(?s).{0,400}", host in ".{0,24}") {
        let _ = check(&text, Path::new("/kh"), &host, &key(KEY_A));
    }
}
