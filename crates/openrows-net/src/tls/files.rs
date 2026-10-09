// SPDX-License-Identifier: Apache-2.0
//! PEM files for TLS: CA bundles, client certificate chains and client keys
//! (PKCS#8, encrypted PKCS#8, PKCS#1, SEC1). Key bytes are zeroized.

use std::{
    fs, io,
    io::Read as _,
    path::{Path, PathBuf},
};

use pkcs8::{EncryptedPrivateKeyInfoRef, SecretDocument};
use rustls::{
    RootCertStore,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, pem::PemObject},
};
use zeroize::Zeroizing;

use super::{CloudCa, TlsError, TlsParams};
use crate::SecretString;

/// Largest PEM file read (CA bundles of public clouds are well below this).
const PEM_LIMIT: u64 = 4 * 1024 * 1024;
const ENCRYPTED_BEGIN: &str = "-----BEGIN ENCRYPTED PRIVATE KEY-----";
const ENCRYPTED_END: &str = "-----END ENCRYPTED PRIVATE KEY-----";

/// A client certificate chain and its private key.
pub(crate) struct ClientIdentity {
    pub(crate) chain: Vec<CertificateDer<'static>>,
    pub(crate) key: PrivateKeyDer<'static>,
    pub(crate) key_path: PathBuf,
}

/// Trust anchors from a PEM CA bundle.
pub(crate) fn ca_roots(path: &Path) -> Result<RootCertStore, TlsError> {
    let error = |reason: String| TlsError::CaFile {
        path: path.to_path_buf(),
        reason,
    };
    let certificates = read_certificates(path).map_err(error)?;
    let mut roots = RootCertStore::empty();
    let (added, _) = roots.add_parsable_certificates(certificates);
    if added == 0 {
        return Err(error("no usable CA certificate".to_owned()));
    }
    Ok(roots)
}

/// Trust anchors from a bundled cloud provider CA set.
pub(crate) fn bundled_roots(cloud: CloudCa) -> Result<RootCertStore, TlsError> {
    let certificates = CertificateDer::pem_slice_iter(cloud.pem())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| TlsError::Config(format!("{cloud}: {error}")))?;
    let mut roots = RootCertStore::empty();
    roots.add_parsable_certificates(certificates);
    Ok(roots)
}

/// The client certificate and key, when both are configured.
pub(crate) fn client_identity(params: &TlsParams) -> Result<Option<ClientIdentity>, TlsError> {
    let (cert_path, key_path) = match (&params.client_cert, &params.client_key) {
        (None, None) => return Ok(None),
        (Some(cert), Some(key)) => (cert, key),
        _ => return Err(TlsError::IncompleteClientIdentity),
    };
    let chain = read_certificates(cert_path).map_err(|reason| TlsError::ClientCert {
        path: cert_path.clone(),
        reason,
    })?;
    let key = read_key(key_path, params.key_passphrase.as_ref())?;
    Ok(Some(ClientIdentity {
        chain,
        key,
        key_path: key_path.clone(),
    }))
}

fn read_certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let bytes = read_limited(path).map_err(|error| error.to_string())?;
    let certificates = CertificateDer::pem_slice_iter(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("not a PEM certificate file ({error})"))?;
    if certificates.is_empty() {
        return Err("no PEM certificate found".to_owned());
    }
    Ok(certificates)
}

/// Read a client key, decrypting encrypted PKCS#8 with `passphrase`.
pub(crate) fn read_key(
    path: &Path,
    passphrase: Option<&SecretString>,
) -> Result<PrivateKeyDer<'static>, TlsError> {
    let key_error = |reason: &str| TlsError::ClientKey {
        path: path.to_path_buf(),
        reason: reason.to_owned(),
    };
    let bytes = read_limited(path).map_err(|error| key_error(&error.to_string()))?;
    let text = std::str::from_utf8(&bytes).map_err(|_| key_error("not a PEM file"))?;
    if let Some(block) = encrypted_block(text) {
        return decrypt(path, block, passphrase);
    }
    if text.contains("Proc-Type: 4,ENCRYPTED") {
        return Err(key_error(
            "legacy OpenSSL key encryption is not supported; convert it with \
             `openssl pkcs8 -topk8 -v2 aes-256-cbc`",
        ));
    }
    PrivateKeyDer::from_pem_slice(&bytes).map_err(|_| key_error("no PEM private key found"))
}

fn encrypted_block(text: &str) -> Option<&str> {
    let start = text.find(ENCRYPTED_BEGIN)?;
    let end = text[start..].find(ENCRYPTED_END)? + start + ENCRYPTED_END.len();
    Some(&text[start..end])
}

fn decrypt(
    path: &Path,
    block: &str,
    passphrase: Option<&SecretString>,
) -> Result<PrivateKeyDer<'static>, TlsError> {
    let passphrase = passphrase.ok_or_else(|| TlsError::KeyPassphraseRequired {
        path: path.to_path_buf(),
    })?;
    let (_, document) = SecretDocument::from_pem(block).map_err(|_| malformed(path))?;
    let info =
        EncryptedPrivateKeyInfoRef::try_from(document.as_bytes()).map_err(|_| malformed(path))?;
    let decrypted =
        info.decrypt(passphrase.expose())
            .map_err(|_| TlsError::KeyPassphraseWrong {
                path: path.to_path_buf(),
            })?;
    Ok(PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
        decrypted.as_bytes().to_vec(),
    )))
}

fn malformed(path: &Path) -> TlsError {
    TlsError::ClientKey {
        path: path.to_path_buf(),
        reason: "malformed encrypted PKCS#8 key".to_owned(),
    }
}

fn read_limited(path: &Path) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::new());
    fs::File::open(path)?
        .take(PEM_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > PEM_LIMIT) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file is larger than 4 MiB",
        ));
    }
    Ok(bytes)
}
