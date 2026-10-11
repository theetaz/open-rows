// SPDX-License-Identifier: Apache-2.0
//! Compressed public points retain their existing OpenSSH compatibility.

use russh::keys::{
    PrivateKey,
    ssh_key::{
        encoding::pem::LineEnding,
        private::{EcdsaKeypair, KeypairData},
    },
};

use super::{DecodeError, decode, fixture, native, verifies};

fn opposite(bytes: &[u8]) -> Vec<u8> {
    let mut bytes = bytes.to_vec();
    bytes[0] ^= 1; // Toggle the compressed point's Y parity.
    bytes
}

fn opposite_key(key: &PrivateKey) -> PrivateKey {
    let mut pair = key.key_data().ecdsa().expect("ECDSA").clone();
    match &mut pair {
        EcdsaKeypair::NistP256 { public, .. } => {
            *public =
                p256::Sec1Point::from_bytes(opposite(public.as_bytes())).expect("opposite point");
        }
        EcdsaKeypair::NistP384 { public, .. } => {
            *public =
                p384::Sec1Point::from_bytes(opposite(public.as_bytes())).expect("opposite point");
        }
        EcdsaKeypair::NistP521 { public, .. } => {
            *public =
                p521::Sec1Point::from_bytes(opposite(public.as_bytes())).expect("opposite point");
        }
    }
    PrivateKey::new(KeypairData::Ecdsa(pair), "TEST ONLY wrong point").expect("test key")
}

#[test]
fn compressed_public_points_decode_and_sign_for_every_curve() {
    for bits in [256, 384, 521] {
        let key = native(&fixture(bits, "full.openssh"), None);
        let mut pair = key.key_data().ecdsa().expect("ECDSA").clone();
        match &mut pair {
            EcdsaKeypair::NistP256 { public, .. } => *public = public.compress(),
            EcdsaKeypair::NistP384 { public, .. } => *public = public.compress(),
            EcdsaKeypair::NistP521 { public, .. } => *public = public.compress(),
        }
        let compressed = PrivateKey::new(KeypairData::Ecdsa(pair), "TEST ONLY compressed")
            .expect("compressed test key");
        let text = compressed.to_openssh(LineEnding::LF).expect("encode");
        let upstream = PrivateKey::from_openssh(&text).expect("upstream supports compression");
        verifies(&upstream);
        let loaded = native(&text, None);
        assert_eq!(loaded.key_data(), upstream.key_data());
        verifies(&loaded);
        let wrong = opposite_key(&compressed)
            .to_openssh(LineEnding::LF)
            .expect("encode opposite point");
        assert!(matches!(decode(&wrong, None), Err(DecodeError::Invalid(_))));
    }
}
