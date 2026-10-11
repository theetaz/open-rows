// SPDX-License-Identifier: Apache-2.0
//! Key formats generated at test time with `ssh-keygen`.
use std::{fs, path::Path, process::Command};

use ring::signature::{
    RSA_PKCS1_2048_8192_SHA256, RSA_PKCS1_2048_8192_SHA512, RsaPublicKeyComponents,
};
use russh::keys::{HashAlg, ssh_key::public::KeyData};

use super::{
    AuthMethod,
    key::{DecodeError, LoadedKey, decode},
    rsa::RsaSignError,
};
use crate::SecretString;

const PASSPHRASE: &str = "synthetic-test-passphrase";

fn keygen(dir: &Path, name: &str, args: &[&str]) -> String {
    let path = dir.join(name);
    let status = Command::new("ssh-keygen")
        .args(["-q", "-C", "", "-f"])
        .arg(&path)
        .args(args)
        .status()
        .expect("ssh-keygen is installed");
    assert!(status.success(), "ssh-keygen {args:?}");
    fs::read_to_string(path).expect("generated key")
}

fn kind(result: Result<LoadedKey, DecodeError>) -> Result<&'static str, DecodeError> {
    result.map(|key| match key {
        LoadedKey::Native(_) => "native",
        LoadedKey::Rsa(_) => "rsa",
    })
}

#[test]
fn decodes_every_supported_format() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cases = [
        ("ed25519", vec!["-t", "ed25519", "-N", ""], "native"),
        ("ecdsa", vec!["-t", "ecdsa", "-N", ""], "native"),
        ("rsa", vec!["-t", "rsa", "-b", "3072", "-N", ""], "rsa"),
        (
            "rsa-pem",
            vec!["-t", "rsa", "-b", "2048", "-m", "PEM", "-N", ""],
            "rsa",
        ),
        (
            "rsa-pkcs8",
            vec!["-t", "rsa", "-b", "2048", "-m", "PKCS8", "-N", ""],
            "rsa",
        ),
        (
            "ecdsa-pem",
            vec!["-t", "ecdsa", "-m", "PEM", "-N", ""],
            "native",
        ),
        (
            "ecdsa-pkcs8",
            vec!["-t", "ecdsa", "-m", "PKCS8", "-N", ""],
            "native",
        ),
    ];
    for (name, args, expected) in cases {
        let text = keygen(dir.path(), name, &args);
        assert_eq!(kind(decode(&text, None)), Ok(expected), "{name}");
    }
}

#[test]
fn encrypted_openssh_keys_need_the_right_passphrase() {
    let dir = tempfile::tempdir().expect("temp dir");
    for (name, algorithm, expected) in [
        ("ed25519-enc", "ed25519", "native"),
        ("rsa-enc", "rsa", "rsa"),
    ] {
        let text = keygen(dir.path(), name, &["-t", algorithm, "-N", PASSPHRASE]);
        assert_eq!(
            kind(decode(&text, None)),
            Err(DecodeError::PassphraseRequired)
        );
        assert!(matches!(
            kind(decode(&text, Some(&"wrong".into()))),
            Err(DecodeError::Invalid(_))
        ));
        assert_eq!(kind(decode(&text, Some(&PASSPHRASE.into()))), Ok(expected));
    }
}

#[test]
fn unsupported_inputs_explain_themselves() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unsupported = [("rsa-pem-enc", ["-t", "rsa", "-m", "PEM", "-N", PASSPHRASE])];
    for (name, args) in unsupported {
        let text = keygen(dir.path(), name, &args);
        let reason = match kind(decode(&text, Some(&PASSPHRASE.into()))) {
            Err(DecodeError::Invalid(reason)) => Some(reason),
            _ => None,
        }
        .expect("unsupported format is rejected");
        assert!(reason.contains("ssh-keygen -p"), "{name}: {reason}");
    }
    assert!(matches!(
        kind(decode("not a key", None)),
        Err(DecodeError::Invalid(_))
    ));
}

#[test]
fn ring_rsa_signatures_verify_against_the_public_key() {
    let dir = tempfile::tempdir().expect("temp dir");
    let text = keygen(
        dir.path(),
        "rsa-sign",
        &["-t", "rsa", "-b", "3072", "-N", ""],
    );
    let signer = match decode(&text, None) {
        Ok(LoadedKey::Rsa(signer)) => Some(signer),
        _ => None,
    }
    .expect("RSA key loads into ring");
    let public = match signer.public_key().key_data() {
        KeyData::Rsa(public) => Some(public.clone()),
        _ => None,
    }
    .expect("RSA public key");
    let components = RsaPublicKeyComponents {
        n: public.n().as_positive_bytes().expect("n"),
        e: public.e().as_positive_bytes().expect("e"),
    };
    let message = b"session-identifier".to_vec();
    for (hash, algorithm, name) in [
        (HashAlg::Sha512, &RSA_PKCS1_2048_8192_SHA512, "rsa-sha2-512"),
        (HashAlg::Sha256, &RSA_PKCS1_2048_8192_SHA256, "rsa-sha2-256"),
    ] {
        let output = signer.sign(Some(hash), message.clone()).expect("signs");
        let blob = &output[message.len() + 4..];
        let name_len =
            usize::try_from(u32::from_be_bytes(blob[..4].try_into().expect("len"))).expect("fits");
        assert_eq!(&blob[4..4 + name_len], name.as_bytes());
        let signature = &blob[4 + name_len + 4..];
        components
            .verify(algorithm, &message, signature)
            .expect("ring verifies the signature");
    }
    assert!(matches!(
        signer.sign(None, message),
        Err(RsaSignError::Sha1Only)
    ));
}

#[test]
fn auth_method_debug_never_shows_secrets() {
    let methods = [
        AuthMethod::Password(SecretString::from("synthetic-password")),
        AuthMethod::PrivateKey {
            path: "/keys/id".into(),
            passphrase: Some(SecretString::from("synthetic-passphrase")),
        },
    ];
    let rendered = format!("{methods:?}");
    assert!(!rendered.contains("synthetic-"), "{rendered}");
}
