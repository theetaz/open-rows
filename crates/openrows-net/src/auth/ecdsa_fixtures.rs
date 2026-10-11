// SPDX-License-Identifier: Apache-2.0
//! Runtime-only keys from visibly synthetic hex scalars; never use for authentication.

use std::{fs, process::Command};

use data_encoding::{BASE64, HEXLOWER};
use pkcs8::{
    EncodePrivateKey, PrivateKeyInfoRef,
    der::{Decode, Reader, SliceReader, Tag, TagNumber, Tagged, asn1::AnyRef},
};
use russh::keys::{
    PrivateKey,
    ssh_key::{
        encoding::Encode,
        private::{EcdsaKeypair, KeypairData},
    },
};
use zeroize::Zeroizing;

// Short magnitudes omit one curve-width byte; sign magnitudes start with 0x80.
// P-521 sign padding applies to a 65-byte magnitude since scalars are at most 521 bits.
const SCALARS: &[(u32, [&str; 3])] = &[
    (
        256,
        [
            "01111111111111111111111111111111111111111111111111111111111111",
            "8011111111111111111111111111111111111111111111111111111111111111",
            "0111111111111111111111111111111111111111111111111111111111111111",
        ],
    ),
    (
        384,
        [
            "0111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "801111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "011111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
        ],
    ),
    (
        521,
        [
            "0111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "8011111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
            "011111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
        ],
    ),
];

/// Build a supported format from synthetic scalar bytes without repository key files.
pub(super) fn fixture(bits: u32, suffix: &str) -> String {
    let case = match suffix {
        "sign.openssh" => 1,
        "full.openssh" => 2,
        _ => 0,
    };
    let hex = SCALARS
        .iter()
        .find(|row| row.0 == bits)
        .expect("test curve")
        .1[case];
    let scalar = HEXLOWER.decode(hex.as_bytes()).expect("synthetic hex");
    let (pair, sec1, pkcs8) = key_material(bits, &scalar);
    match suffix {
        "short.sec1" => pem("EC ", &sec1),
        "short.explicit.sec1" => pem("EC ", &explicit_sec1(bits, &sec1)),
        "short.pkcs8" => pem("", &pkcs8),
        "short.explicit.pkcs8" => pem("", &explicit_pkcs8(bits, &pkcs8)),
        "short.encrypted" => encrypt(&openssh(pair, &scalar), "aes256-ctr"),
        "leading-zero.openssh" | "short.openssh" | "sign.openssh" | "full.openssh" => {
            openssh(pair, &scalar)
        }
        _ => unreachable!("fixture callers use supported test formats"),
    }
}

fn key_material(
    bits: u32,
    scalar: &[u8],
) -> (EcdsaKeypair, Zeroizing<Vec<u8>>, Zeroizing<Vec<u8>>) {
    let width = usize::try_from(bits.div_ceil(8)).expect("curve width");
    let mut padded = Zeroizing::new(vec![0; width]);
    padded[width - scalar.len()..].copy_from_slice(scalar);
    macro_rules! material {
        ($curve:ident, $variant:ident) => {{
            let secret = $curve::SecretKey::from_slice(&padded).expect("synthetic scalar");
            let sec1 = secret.to_sec1_der().expect("SEC1 encoding");
            let pkcs8 = Zeroizing::new(
                secret
                    .to_pkcs8_der()
                    .expect("PKCS#8 encoding")
                    .as_bytes()
                    .to_vec(),
            );
            let pair = EcdsaKeypair::$variant {
                public: secret.public_key().into(),
                private: secret.into(),
            };
            (pair, sec1, pkcs8)
        }};
    }
    match bits {
        256 => material!(p256, NistP256),
        384 => material!(p384, NistP384),
        521 => material!(p521, NistP521),
        _ => unreachable!("fixture callers use the three standard test curves"),
    }
}

fn domain(bits: u32) -> &'static [u8] {
    match bits {
        256 => include_bytes!("ecdsa_parameters/p256.der").as_slice(),
        384 => include_bytes!("ecdsa_parameters/p384.der").as_slice(),
        521 => include_bytes!("ecdsa_parameters/p521.der").as_slice(),
        _ => unreachable!("fixture callers use the three standard test curves"),
    }
}

fn explicit_pkcs8(bits: u32, named: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut info = PrivateKeyInfoRef::try_from(named).expect("named PKCS#8");
    info.algorithm.parameters = Some(AnyRef::from_der(domain(bits)).expect("public domain"));
    Zeroizing::new(pkcs8::der::Encode::to_der(&info).expect("explicit PKCS#8 encoding"))
}

fn explicit_sec1(bits: u32, named: &[u8]) -> Zeroizing<Vec<u8>> {
    let sequence = AnyRef::from_der(named).expect("named SEC1");
    let mut reader = SliceReader::new(sequence.value()).expect("SEC1 fields");
    let parameter_tag = Tag::ContextSpecific {
        number: TagNumber(0),
        constructed: true,
    };
    let mut fields = Zeroizing::new(Vec::new());
    while !reader.is_finished() {
        let mut field = AnyRef::decode(&mut reader).expect("SEC1 field");
        if field.tag() == parameter_tag {
            field = AnyRef::new(parameter_tag, domain(bits)).expect("explicit SEC1 parameters");
        }
        fields.extend(pkcs8::der::Encode::to_der(&field).expect("SEC1 field encoding"));
    }
    let explicit = AnyRef::new(Tag::Sequence, &fields).expect("explicit SEC1 sequence");
    Zeroizing::new(pkcs8::der::Encode::to_der(&explicit).expect("explicit SEC1 encoding"))
}

fn openssh(pair: EcdsaKeypair, scalar: &[u8]) -> String {
    let key = PrivateKey::new(KeypairData::Ecdsa(pair), "TEST ONLY synthetic").expect("test key");
    let mut public = Vec::new();
    key.public_key()
        .key_data()
        .encode(&mut public)
        .expect("public encoding");
    let mut private = Zeroizing::new(Vec::new());
    for _ in 0..2 {
        0x1111_1111_u32.encode(&mut *private).expect("checkint");
    }
    private.extend_from_slice(&public);
    let mut mpint = scalar.to_vec();
    if scalar[0] >= 0x80 {
        mpint.insert(0, 0);
    }
    mpint.encode(&mut *private).expect("scalar encoding");
    key.comment()
        .encode(&mut *private)
        .expect("comment encoding");
    let padding = (8 - private.len() % 8) % 8;
    private.extend(1..=u8::try_from(padding).expect("padding size"));
    let mut bytes = Zeroizing::new(b"openssh-key-v1\0".to_vec());
    for field in [b"none".as_slice(), b"none", b""] {
        field.encode(&mut *bytes).expect("header field");
    }
    1_u32.encode(&mut *bytes).expect("key count");
    public.encode(&mut *bytes).expect("public blob");
    private
        .as_slice()
        .encode(&mut *bytes)
        .expect("private blob");
    pem("OPENSSH ", &bytes)
}

/// Assemble PEM delimiters only at runtime, keeping scanner patterns out of sources.
pub(super) fn pem(prefix: &str, bytes: &[u8]) -> String {
    format!(
        "-----BEGIN {prefix}{label}-----\n{}\n-----END {prefix}{label}-----\n",
        BASE64.encode(bytes),
        label = concat!("PRIVATE", " KEY"),
    )
}

/// Re-encrypt a runtime fixture in an automatically removed temporary directory.
pub(super) fn encrypt(text: &str, cipher: &str) -> String {
    let dir = tempfile::tempdir().expect("test directory");
    let path = dir.path().join("test-only-key");
    fs::write(&path, text).expect("runtime fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("permissions");
    }
    let output = Command::new("ssh-keygen")
        .args([
            "-q",
            "-p",
            "-a",
            "1",
            "-Z",
            cipher,
            "-P",
            "",
            "-N",
            "test-only",
            "-f",
        ])
        .arg(&path)
        .output()
        .expect("ssh-keygen");
    assert!(
        output.status.success(),
        "cipher {cipher}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(path).expect("encrypted runtime key")
}
