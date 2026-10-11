// SPDX-License-Identifier: Apache-2.0
//! Runtime synthetic ECDSA fixtures; never use these keys for authentication.
use std::{fs, process::Command};

use russh::keys::{
    HashAlg, PrivateKey,
    ssh_key::{
        encoding::{Decode, Encode, pem::LineEnding},
        private::KeypairData,
        public::KeyData,
    },
};
use zeroize::Zeroizing;

use super::key::{DecodeError, LoadedKey, decode, pem_body};

use super::ecdsa_fixtures::{encrypt, fixture, pem};

#[path = "ecdsa_point_tests.rs"]
mod point_tests;

fn native(text: &str, password: Option<&str>) -> std::sync::Arc<PrivateKey> {
    let loaded = decode(text, password.map(Into::into).as_ref()).expect("ECDSA test key decodes");
    match loaded {
        LoadedKey::Native(key) => Some(key),
        LoadedKey::Rsa(_) => None,
    }
    .expect("ECDSA uses the native signer")
}

fn verifies(key: &PrivateKey) {
    let message = b"test-only ECDSA signing regression";
    let signature = key
        .sign("openrows-test", HashAlg::Sha256, message)
        .expect("sign");
    key.public_key()
        .verify("openrows-test", message, &signature)
        .expect("verify");
}

#[test]
fn short_ecdsa_scalars_decode() {
    for bits in [256, 384, 521] {
        let text = fixture(bits, "short.openssh");
        if bits == 256 {
            assert!(matches!(
                PrivateKey::from_openssh(&text),
                Err(russh::keys::ssh_key::Error::Encoding(_))
            ));
        }
        let key = native(&text, None);
        verifies(&key);
        let scalar = key.key_data().ecdsa().expect("ECDSA").private_key_bytes();
        assert_eq!(
            scalar.len(),
            usize::try_from(bits.div_ceil(8)).expect("size")
        );
        assert_eq!(scalar[0], 0, "scalar padded to curve width");
    }
}

#[test]
fn leading_zero_and_fixed_width_scalar_serializations_decode() {
    verifies(&native(&fixture(256, "leading-zero.openssh"), None));
    for bits in [256, 384, 521] {
        let key = native(&fixture(bits, "short.openssh"), None);
        let encoded = key.to_openssh(LineEnding::LF).expect("serialize");
        assert_eq!(*native(&encoded, None), *key);
    }
}

#[test]
fn sign_padded_and_full_width_ecdsa_scalars_decode() {
    for bits in [256, 384, 521] {
        for suffix in ["sign.openssh", "full.openssh"] {
            let text = fixture(bits, suffix);
            verifies(&native(&text, None));
            // Existing upstream-compatible encodings retain the exact key material.
            assert_eq!(
                *native(&text, None),
                PrivateKey::from_openssh(&text).expect("upstream")
            );
        }
    }
}

#[test]
fn pem_and_pkcs8_ecdsa_keys_match_openssh_for_every_curve() {
    for bits in [256, 384, 521] {
        let openssh = native(&fixture(bits, "short.openssh"), None);
        for suffix in [
            "short.sec1",
            "short.explicit.sec1",
            "short.pkcs8",
            "short.explicit.pkcs8",
        ] {
            for password in [None, Some("unused-test-only")] {
                let key = native(&fixture(bits, suffix), password);
                assert_eq!(key.key_data(), openssh.key_data());
                verifies(&key);
            }
        }
    }
}

#[test]
fn encrypted_short_ecdsa_keys_require_the_correct_passphrase() {
    for bits in [256, 384, 521] {
        let text = fixture(bits, "short.encrypted");
        assert!(matches!(
            decode(&text, None),
            Err(DecodeError::PassphraseRequired)
        ));
        assert!(matches!(
            decode(&text, Some(&"wrong".into())),
            Err(DecodeError::Invalid(_))
        ));
        let key = native(&text, Some("test-only"));
        assert_eq!(
            key.key_data(),
            native(&fixture(bits, "short.openssh"), None).key_data()
        );
        verifies(&key);
    }
}

#[test]
fn encrypted_ecdsa_cipher_variants_decode_and_authenticate() {
    for cipher in [
        "aes256-cbc",
        "aes256-gcm@openssh.com",
        "chacha20-poly1305@openssh.com",
    ] {
        let text = Zeroizing::new(encrypt(&fixture(256, "short.openssh"), cipher));
        verifies(&native(&text, Some("test-only")));
        assert!(matches!(
            decode(&text, Some(&"wrong".into())),
            Err(DecodeError::Invalid(_))
        ));
    }
}

fn private_parts() -> (Vec<u8>, Zeroizing<Vec<u8>>) {
    let bytes = pem_body(&fixture(256, "short.openssh")).expect("fixture PEM");
    let mut reader = &bytes[15..];
    for _ in 0..3 {
        Vec::<u8>::decode(&mut reader).expect("header field");
    }
    u32::decode(&mut reader).expect("key count");
    Vec::<u8>::decode(&mut reader).expect("public key");
    let offset = bytes.len() - reader.len();
    let private = Zeroizing::new(Vec::<u8>::decode(&mut reader).expect("private blob"));
    (bytes[..offset].to_vec(), private)
}

fn scalar_bounds(private: &[u8]) -> (usize, usize) {
    let mut reader = &private[8..];
    KeyData::decode(&mut reader).expect("public key");
    let start = private.len() - reader.len();
    Vec::<u8>::decode(&mut reader).expect("scalar");
    (start, private.len() - reader.len())
}

fn envelope(header: &[u8], private: &[u8]) -> String {
    let mut bytes = Zeroizing::new(header.to_vec());
    private.to_vec().encode(&mut *bytes).expect("encode");
    pem("OPENSSH ", &bytes)
}

fn with_scalar(scalar: &[u8]) -> String {
    let (header, private) = private_parts();
    let (start, end) = scalar_bounds(&private);
    let mut tail = &private[end..];
    String::decode(&mut tail).expect("comment");
    let comment_end = private.len() - tail.len();
    let mut changed = Zeroizing::new(private[..start].to_vec());
    scalar.to_vec().encode(&mut *changed).expect("scalar");
    changed.extend_from_slice(&private[end..comment_end]);
    let padding = (8 - changed.len() % 8) % 8;
    changed.extend(1..=u8::try_from(padding).expect("padding"));
    envelope(&header, &changed)
}

#[test]
fn invalid_scalar_encodings_and_mismatched_public_keys_are_rejected() {
    for scalar in [
        vec![],
        vec![0],
        vec![0x80],
        vec![0, 1],
        vec![1; 33],
        vec![0, 0, 0x80],
    ] {
        assert!(matches!(
            decode(&with_scalar(&scalar), None),
            Err(DecodeError::Invalid(_))
        ));
    }
    // In-range, canonical scalar with the wrong public point.
    assert!(matches!(
        decode(&with_scalar(&[1]), None),
        Err(DecodeError::Invalid(_))
    ));
    // P-256 order and larger values cannot be private scalars.
    assert!(matches!(
        decode(
            &with_scalar(&[0, 0xff].into_iter().chain([0xff; 31]).collect::<Vec<_>>()),
            None
        ),
        Err(DecodeError::Invalid(_))
    ));
}

#[test]
fn malformed_envelopes_checks_and_padding_are_rejected() {
    let (header, private) = private_parts();
    for offset in [0, private.len() - 1] {
        let mut changed = private.clone();
        changed[offset] ^= 1;
        assert!(matches!(
            decode(&envelope(&header, &changed), None),
            Err(DecodeError::Invalid(_))
        ));
    }
    let mut header = header;
    header[0] ^= 1;
    assert!(matches!(
        decode(&envelope(&header, &private), None),
        Err(DecodeError::Invalid(_))
    ));
    let mut bytes = pem_body(&fixture(256, "short.openssh")).expect("fixture");
    bytes.push(0);
    let text = pem("OPENSSH ", &bytes);
    assert!(matches!(decode(&text, None), Err(DecodeError::Invalid(_))));
}

#[test]
#[ignore = "opt-in: generates 200 keys per curve with ssh-keygen"]
fn generated_ecdsa_keys_decode() {
    let dir = tempfile::tempdir().expect("test directory");
    for bits in ["256", "384", "521"] {
        for _ in 0..200 {
            let path = dir.path().join("test-only-key");
            let status = Command::new("ssh-keygen")
                .args(["-q", "-t", "ecdsa", "-b", bits, "-N", ""])
                .args(["-C", "TEST ONLY", "-f"])
                .arg(&path)
                .status()
                .expect("ssh-keygen");
            assert!(status.success());
            let text = Zeroizing::new(fs::read_to_string(&path).expect("test key"));
            let key = native(&text, None);
            assert!(matches!(key.key_data(), KeypairData::Ecdsa(_)));
            fs::remove_file(&path).expect("remove key");
            fs::remove_file(path.with_extension("pub")).expect("remove public key");
        }
    }
}
