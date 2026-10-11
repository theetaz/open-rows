// SPDX-License-Identifier: Apache-2.0
//! OpenSSH ECDSA decoding with unsigned, curve-width scalar padding.

use russh::keys::{
    PrivateKey,
    ssh_key::{
        self, Cipher, Kdf,
        encoding::{Decode, Reader},
        private::{EcdsaKeypair, KeypairData},
        public::{EcdsaPublicKey, KeyData},
    },
};
use zeroize::Zeroizing;

use super::key::{DecodeError, pem_body};
use crate::SecretString;

/// Decode OpenSSH ECDSA keys; leave other algorithms to the existing loader.
pub(super) fn decode(
    text: &str,
    passphrase: Option<&SecretString>,
) -> Result<Option<PrivateKey>, DecodeError> {
    let bytes = pem_body(text)?;
    decode_openssh(&bytes, passphrase).map_err(|error| match error {
        ssh_key::Error::Encrypted => DecodeError::PassphraseRequired,
        error => DecodeError::Invalid(error.to_string()),
    })
}

fn decode_openssh(
    mut bytes: &[u8],
    passphrase: Option<&SecretString>,
) -> ssh_key::Result<Option<PrivateKey>> {
    let mut magic = [0; 15];
    bytes.read(&mut magic)?;
    if &magic != b"openssh-key-v1\0" {
        return Err(ssh_key::Error::FormatEncoding);
    }
    let cipher = Cipher::decode(&mut bytes)?;
    let kdf = Kdf::decode(&mut bytes)?;
    if u32::decode(&mut bytes)? != 1 {
        return Err(ssh_key::Error::FormatEncoding);
    }
    let public = bytes.read_prefixed(|reader| {
        let public = KeyData::decode(reader)?;
        reader.finish(public).map_err(ssh_key::Error::from)
    })?;
    let KeyData::Ecdsa(public) = public else {
        return Ok(None);
    };
    let mut private = Zeroizing::new(Vec::<u8>::decode(&mut bytes)?);
    decrypt(
        &mut private,
        &mut bytes,
        &Encryption { cipher, kdf },
        passphrase,
    )?;
    decode_private(&private, public, cipher).map(Some)
}

struct Encryption {
    cipher: Cipher,
    kdf: Kdf,
}

fn decrypt(
    private: &mut [u8],
    bytes: &mut &[u8],
    encryption: &Encryption,
    passphrase: Option<&SecretString>,
) -> ssh_key::Result<()> {
    let Encryption { cipher, kdf } = encryption;
    let cipher = *cipher;
    if private.len().checked_rem(cipher.block_size()) != Some(0) {
        return Err(ssh_key::Error::FormatEncoding);
    }
    let tag = if cipher.has_tag() {
        let mut tag = ssh_key::cipher::Tag::default();
        bytes.read(&mut tag)?;
        Some(tag)
    } else {
        None
    };
    bytes.ensure_finished()?;
    if cipher.is_none() {
        return if kdf.is_none() {
            Ok(())
        } else {
            Err(ssh_key::Error::Crypto)
        };
    }
    let password = passphrase.ok_or(ssh_key::Error::Encrypted)?;
    let (key, iv) = kdf.derive_key_and_iv(cipher, password.expose())?;
    cipher.decrypt(&key, &iv, private, tag)?;
    Ok(())
}

fn decode_private(
    mut bytes: &[u8],
    public: EcdsaPublicKey,
    cipher: Cipher,
) -> ssh_key::Result<PrivateKey> {
    if u32::decode(&mut bytes)? != u32::decode(&mut bytes)? {
        return Err(ssh_key::Error::Crypto);
    }
    if KeyData::decode(&mut bytes)? != KeyData::Ecdsa(public) {
        return Err(ssh_key::Error::PublicKey);
    }
    let scalar = Zeroizing::new(Vec::<u8>::decode(&mut bytes)?);
    let pair = keypair(public, &scalar)?;
    let comment = String::decode(&mut bytes)?;
    validate_padding(bytes, cipher)?;
    PrivateKey::new(KeypairData::Ecdsa(pair), comment)
}

fn validate_padding(bytes: &[u8], cipher: Cipher) -> ssh_key::Result<()> {
    let max_padding = if cipher.is_none() {
        16
    } else {
        cipher.block_size().saturating_sub(1)
    };
    let length = u8::try_from(bytes.len()).map_err(|_| ssh_key::Error::FormatEncoding)?;
    if bytes.len() > max_padding || !bytes.iter().copied().eq(1..=length) {
        return Err(ssh_key::Error::FormatEncoding);
    }
    Ok(())
}

fn positive_scalar(bytes: &[u8], width: usize) -> ssh_key::Result<&[u8]> {
    if bytes.first().is_none_or(|first| *first >= 0x80) {
        return Err(ssh_key::Error::FormatEncoding);
    }
    let first = bytes
        .iter()
        .position(|byte| *byte != 0)
        .ok_or(ssh_key::Error::Crypto)?;
    // ssh-key's writer emits fixed-width scalars, including redundant zero padding.
    // Otherwise accept only canonical positive mpints (one necessary sign byte).
    if first != 0 && bytes.len() != width && (first != 1 || bytes[first] < 0x80) {
        return Err(ssh_key::Error::FormatEncoding);
    }
    Ok(&bytes[first..])
}

fn padded<const SIZE: usize>(scalar: &[u8]) -> ssh_key::Result<Zeroizing<[u8; SIZE]>> {
    let scalar = positive_scalar(scalar, SIZE)?;
    let offset = SIZE
        .checked_sub(scalar.len())
        .ok_or(ssh_key::Error::FormatEncoding)?;
    let mut bytes = Zeroizing::new([0; SIZE]);
    bytes[offset..].copy_from_slice(scalar);
    Ok(bytes)
}

fn keypair(public: EcdsaPublicKey, scalar: &[u8]) -> ssh_key::Result<EcdsaKeypair> {
    match public {
        EcdsaPublicKey::NistP256(public) => {
            let secret = p256::SecretKey::from_slice(&*padded::<32>(scalar)?)
                .map_err(|_| ssh_key::Error::Crypto)?;
            let derived: p256::Sec1Point = secret.public_key().into();
            let public = matching_public(public, &derived, &derived.compress())?;
            Ok(EcdsaKeypair::NistP256 {
                public,
                private: secret.into(),
            })
        }
        EcdsaPublicKey::NistP384(public) => {
            let secret = p384::SecretKey::from_slice(&*padded::<48>(scalar)?)
                .map_err(|_| ssh_key::Error::Crypto)?;
            let derived: p384::Sec1Point = secret.public_key().into();
            let public = matching_public(public, &derived, &derived.compress())?;
            Ok(EcdsaKeypair::NistP384 {
                public,
                private: secret.into(),
            })
        }
        EcdsaPublicKey::NistP521(public) => {
            let secret = p521::SecretKey::from_slice(&*padded::<66>(scalar)?)
                .map_err(|_| ssh_key::Error::Crypto)?;
            let derived: p521::Sec1Point = secret.public_key().into();
            let public = matching_public(public, &derived, &derived.compress())?;
            Ok(EcdsaKeypair::NistP521 {
                public,
                private: secret.into(),
            })
        }
    }
}

fn matching_public<T: PartialEq>(public: T, derived: &T, compressed: &T) -> ssh_key::Result<T> {
    // Compare both encodings of the validated point, preserving the supplied form.
    if &public != derived && &public != compressed {
        return Err(ssh_key::Error::PublicKey);
    }
    Ok(public)
}
