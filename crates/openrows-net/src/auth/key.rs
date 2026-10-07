// SPDX-License-Identifier: Apache-2.0
//! Private-key file loading: OpenSSH (optionally passphrase-encrypted), PEM
//! PKCS#1 RSA and unencrypted PKCS#8. File text and DER are zeroized.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use data_encoding::BASE64;
use russh::keys::{PrivateKey, decode_secret_key};
use zeroize::Zeroizing;

use super::rsa::RsaSigner;
use crate::{SecretString, SshError, blocking::off_runtime};

const PKCS1_RSA: &str = "-----BEGIN RSA PRIVATE KEY-----";
const PKCS8: &str = "-----BEGIN PRIVATE KEY-----";

/// A decoded key, signed by russh (ed25519/ECDSA) or ring (RSA).
pub(crate) enum LoadedKey {
    Native(Arc<PrivateKey>),
    Rsa(Box<RsaSigner>),
}

/// Why decoding failed, mapped to [`SshError`] with the file path.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum DecodeError {
    PassphraseRequired,
    Invalid(String),
}

/// Read and decode `path` off the async runtime (bcrypt-pbkdf is CPU-heavy).
pub(crate) async fn load(
    path: &Path,
    passphrase: Option<&SecretString>,
) -> Result<LoadedKey, SshError> {
    let path = path.to_path_buf();
    let passphrase = passphrase.cloned();
    off_runtime(move || load_blocking(&path, passphrase.as_ref())).await
}

fn load_blocking(path: &Path, passphrase: Option<&SecretString>) -> Result<LoadedKey, SshError> {
    let text = Zeroizing::new(
        fs::read_to_string(path).map_err(|error| key_error(path, error.to_string()))?,
    );
    decode(&text, passphrase).map_err(|error| match error {
        DecodeError::PassphraseRequired => SshError::PassphraseRequired {
            path: path.to_path_buf(),
        },
        DecodeError::Invalid(reason) => key_error(path, reason),
    })
}

/// Decode key file text; pure, so formats are unit-testable without files.
pub(crate) fn decode(
    text: &str,
    passphrase: Option<&SecretString>,
) -> Result<LoadedKey, DecodeError> {
    if text.contains(PKCS1_RSA) {
        if text.contains("ENCRYPTED") {
            return Err(DecodeError::Invalid(
                "encrypted PKCS#1 PEM keys are not supported; re-encrypt in OpenSSH format with `ssh-keygen -p`".into(),
            ));
        }
        let der = pem_body(text)?;
        return rsa_key(RsaSigner::from_pkcs1_der(&der));
    }
    match decode_secret_key(text, passphrase.map(SecretString::expose)) {
        Ok(key) if key.algorithm().is_rsa() => rsa_key(RsaSigner::from_openssh(&key)),
        Ok(key) => Ok(LoadedKey::Native(Arc::new(key))),
        Err(russh::keys::Error::KeyIsEncrypted) => Err(DecodeError::PassphraseRequired),
        // russh reads ed25519 PKCS#8; RSA goes to ring. Other PKCS#8 keys (ECDSA) are not supported yet.
        Err(_) if text.contains(PKCS8) => {
            let der = pem_body(text)?;
            rsa_key(RsaSigner::from_pkcs8_der(&der)).map_err(|_| {
                DecodeError::Invalid(
                    "unsupported PKCS#8 key; convert to OpenSSH format with `ssh-keygen -p`".into(),
                )
            })
        }
        Err(error) => Err(DecodeError::Invalid(error.to_string())),
    }
}

fn rsa_key(signer: Result<RsaSigner, String>) -> Result<LoadedKey, DecodeError> {
    signer
        .map(|signer| LoadedKey::Rsa(Box::new(signer)))
        .map_err(DecodeError::Invalid)
}

/// Base64 body of a PEM block, skipping `Key: value` headers.
fn pem_body(text: &str) -> Result<Zeroizing<Vec<u8>>, DecodeError> {
    let body: Zeroizing<String> = Zeroizing::new(
        text.lines()
            .map(str::trim)
            .skip_while(|line| !line.starts_with("-----BEGIN"))
            .skip(1)
            .take_while(|line| !line.starts_with("-----END"))
            .filter(|line| !line.contains(':'))
            .collect(),
    );
    BASE64
        .decode(body.as_bytes())
        .map(Zeroizing::new)
        .map_err(|_| DecodeError::Invalid("malformed PEM body".into()))
}

fn key_error(path: &Path, reason: String) -> SshError {
    SshError::KeyLoad {
        path: PathBuf::from(path),
        reason,
    }
}
