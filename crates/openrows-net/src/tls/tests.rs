// SPDX-License-Identifier: Apache-2.0
//! Handshakes against an in-memory rustls server with certificates generated
//! at test time: every verification level, client certificates and key files.
#![allow(
    clippy::expect_used,
    reason = "test fixtures fail fast with a message, like the tests that call them"
)]

use std::{path::PathBuf, sync::Arc};

use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, KeyPair, date_time_ymd};
use rustls::{
    RootCertStore, ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    server::WebPkiClientVerifier,
};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};
use tokio_rustls::TlsAcceptor;

use super::{HandshakeFailure, TlsClient, TlsError, TlsParams, Verification};
use crate::SecretString;

/// A CA, a server certificate for `db.test` and the files a profile points at.
struct Pki {
    dir: TempDir,
    ca: CertifiedIssuer<'static, KeyPair>,
}

impl Pki {
    fn new() -> Self {
        let mut params = CertificateParams::new(Vec::<String>::new()).expect("params");
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "OpenRows Test CA");
        let ca =
            CertifiedIssuer::self_signed(params, KeyPair::generate().expect("key")).expect("CA");
        let pki = Self {
            dir: tempfile::tempdir().expect("temp dir"),
            ca,
        };
        pki.write("ca.pem", &pki.ca.pem());
        pki
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, text).expect("write fixture");
        path
    }

    /// A leaf for `name` signed by the CA: (certificate DER, key).
    fn leaf(&self, name: &str, expired: bool) -> (CertificateDer<'static>, KeyPair) {
        let mut params = CertificateParams::new(vec![name.to_owned()]).expect("params");
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, name);
        if expired {
            params.not_before = date_time_ymd(2000, 1, 1);
            params.not_after = date_time_ymd(2001, 1, 1);
        }
        let key = KeyPair::generate().expect("key");
        let certificate = params.signed_by(&key, &self.ca).expect("sign");
        (certificate.der().clone(), key)
    }

    fn params(&self, verification: Verification) -> TlsParams {
        TlsParams {
            ca_file: Some(self.path("ca.pem")),
            ..TlsParams::new(verification)
        }
    }
}

fn server_config(
    certificate: CertificateDer<'static>,
    key: &KeyPair,
    client_ca: Option<&Pki>,
) -> Arc<ServerConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = ServerConfig::builder_with_provider(Arc::clone(&provider))
        .with_safe_default_protocol_versions()
        .expect("versions");
    let builder = match client_ca {
        Some(pki) => {
            let mut roots = RootCertStore::empty();
            roots.add(pki.ca.der().clone()).expect("root");
            let verifier = WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider)
                .build()
                .expect("client verifier");
            builder.with_client_cert_verifier(verifier)
        }
        None => builder.with_no_client_auth(),
    };
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der()));
    Arc::new(
        builder
            .with_single_cert(vec![certificate], key)
            .expect("server config"),
    )
}

/// Handshake `client` against `server` over an in-memory pipe and echo a byte.
async fn handshake(
    client: &TlsClient,
    server: Arc<ServerConfig>,
    host: &str,
) -> Result<super::TlsDetails, TlsError> {
    let (near, far) = duplex(16 * 1024);
    let accept = tokio::spawn(async move {
        let mut stream = TlsAcceptor::from(server).accept(far).await.ok()?;
        let mut byte = [0];
        stream.read_exact(&mut byte).await.ok()?;
        stream.write_all(&byte).await.ok()?;
        stream.flush().await.ok()
    });
    let (mut stream, details) = client.handshake(near, host).await?;
    // TLS 1.3 servers reject a client certificate after the client finished,
    // so that failure only surfaces on the first read.
    let mut echoed = [0];
    let echo = async {
        stream.write_all(b"x").await?;
        stream.read_exact(&mut echoed).await
    };
    echo.await.map_err(|source| TlsError::Io {
        host: host.to_owned(),
        source,
    })?;
    assert_eq!(&echoed, b"x");
    drop(accept.await);
    Ok(details)
}

fn server_for(pki: &Pki, name: &str) -> Arc<ServerConfig> {
    let (certificate, key) = pki.leaf(name, false);
    server_config(certificate, &key, None)
}

fn failure(result: Result<super::TlsDetails, TlsError>) -> HandshakeFailure {
    match result {
        Err(TlsError::Handshake { failure, .. }) => failure,
        other => unreachable!("expected a handshake failure, got {other:?}"),
    }
}

#[tokio::test]
async fn verify_full_with_a_ca_file_reports_the_negotiated_session() {
    let pki = Pki::new();
    let client = TlsClient::new(&pki.params(Verification::Full)).expect("client");
    let details = handshake(&client, server_for(&pki, "db.test"), "db.test")
        .await
        .expect("handshake");
    assert_eq!(details.protocol, "TLS 1.3");
    assert!(details.cipher.starts_with("TLS13_"), "{}", details.cipher);
    assert_eq!(details.subject.as_deref(), Some("CN=db.test"));
    assert_eq!(details.issuer.as_deref(), Some("CN=OpenRows Test CA"));
    assert!(details.not_before.is_some() && details.not_after.is_some());
    assert_eq!(details.alt_names, ["db.test"]);
    let days = details.expires_in_days.expect("validity parsed");
    assert!(days > 30, "rcgen leaves are valid for years: {days}");
    assert_eq!(details.verification, Verification::Full);
}

#[tokio::test]
async fn a_cloud_ca_replaces_the_system_store_and_rejects_other_issuers() {
    let stranger = Pki::new();
    for cloud in super::CloudCa::ALL {
        let params = TlsParams {
            cloud_ca: Some(cloud),
            ..TlsParams::new(Verification::Chain)
        };
        let client = TlsClient::new(&params).expect("bundled roots parse");
        let result = handshake(&client, server_for(&stranger, "db.test"), "db.test").await;
        assert_eq!(failure(result), HandshakeFailure::UntrustedCertificate);
    }
    let aws = super::files::bundled_roots(super::CloudCa::AwsRds).expect("roots");
    let azure = super::files::bundled_roots(super::CloudCa::Azure).expect("roots");
    assert!(aws.len() >= 100, "every RDS region: {}", aws.len());
    assert_eq!(azure.len(), 3);
}

#[tokio::test]
async fn verify_full_rejects_another_host_name_and_verify_ca_accepts_it() {
    let pki = Pki::new();
    let full = TlsClient::new(&pki.params(Verification::Full)).expect("client");
    let result = handshake(&full, server_for(&pki, "db.test"), "other.test").await;
    assert_eq!(failure(result), HandshakeFailure::HostnameMismatch);
    let chain = TlsClient::new(&pki.params(Verification::Chain)).expect("client");
    let details = handshake(&chain, server_for(&pki, "db.test"), "other.test")
        .await
        .expect("chain only ignores the name");
    assert_eq!(details.verification, Verification::Chain);
}

#[tokio::test]
async fn certificates_from_another_ca_are_untrusted_in_both_verify_modes() {
    let pki = Pki::new();
    let stranger = Pki::new();
    for verification in [Verification::Chain, Verification::Full] {
        let client = TlsClient::new(&pki.params(verification)).expect("client");
        let result = handshake(&client, server_for(&stranger, "db.test"), "db.test").await;
        assert_eq!(failure(result), HandshakeFailure::UntrustedCertificate);
    }
}

#[tokio::test]
async fn verify_full_without_a_ca_file_uses_the_system_store() {
    let pki = Pki::new();
    let client = TlsClient::new(&TlsParams::new(Verification::Full)).expect("client");
    let result = handshake(&client, server_for(&pki, "db.test"), "db.test").await;
    assert!(
        matches!(result, Err(TlsError::Handshake { .. })),
        "a private CA is not in the system store: {result:?}"
    );
}

#[tokio::test]
async fn expired_certificates_are_named() {
    let pki = Pki::new();
    let (certificate, key) = pki.leaf("db.test", true);
    let client = TlsClient::new(&pki.params(Verification::Full)).expect("client");
    let result = handshake(&client, server_config(certificate, &key, None), "db.test").await;
    assert_eq!(failure(result), HandshakeFailure::Expired);
}

#[tokio::test]
async fn skip_verification_accepts_any_certificate_but_still_encrypts() {
    let stranger = Pki::new();
    let client = TlsClient::new(&TlsParams::new(Verification::None)).expect("client");
    assert!(client.verification().is_skipped());
    let details = handshake(&client, server_for(&stranger, "db.test"), "10.0.0.1")
        .await
        .expect("unverified handshake");
    assert_eq!(details.verification, Verification::None);
    assert!(!details.cipher.is_empty());
}

#[test]
fn verify_ca_needs_a_ca_file() {
    let result = TlsClient::new(&TlsParams::new(Verification::Chain));
    assert!(matches!(result, Err(TlsError::ChainNeedsCa)));
}

#[test]
fn unreadable_or_empty_ca_files_are_typed() {
    let pki = Pki::new();
    let missing = TlsParams {
        ca_file: Some(pki.path("missing.pem")),
        ..TlsParams::new(Verification::Full)
    };
    assert!(matches!(
        TlsClient::new(&missing),
        Err(TlsError::CaFile { .. })
    ));
    let empty = TlsParams {
        ca_file: Some(pki.write("empty.pem", "not a certificate\n")),
        ..TlsParams::new(Verification::Full)
    };
    let error = TlsClient::new(&empty).err().expect("rejected");
    assert!(error.to_string().contains("no PEM certificate"), "{error}");
}

/// A CA-signed client identity; the key optionally encrypted.
fn client_identity(pki: &Pki, passphrase: Option<&str>) -> TlsParams {
    let (certificate, key) = pki.leaf("app_user", false);
    let certificate_pem = pem_encode("CERTIFICATE", certificate.as_ref());
    let key_pem = match passphrase {
        None => key.serialize_pem(),
        Some(passphrase) => encrypted_pkcs8(&key, passphrase),
    };
    TlsParams {
        client_cert: Some(pki.write("client.pem", &certificate_pem)),
        client_key: Some(pki.write("client.key", &key_pem)),
        ..pki.params(Verification::Full)
    }
}

fn pem_encode(label: &str, der: &[u8]) -> String {
    pkcs8::der::pem::encode_string(label, pkcs8::LineEnding::LF, der).expect("pem")
}

fn encrypted_pkcs8(key: &KeyPair, passphrase: &str) -> String {
    let parameters =
        pkcs8::pkcs5::pbes2::Parameters::generate_pbkdf2_sha256_aes256cbc(2048, &[7; 16], [9; 16])
            .expect("parameters");
    let der = key.serialize_der();
    let info = pkcs8::PrivateKeyInfoRef::try_from(der.as_slice()).expect("pkcs8");
    let encrypted = info
        .encrypt_with_params(parameters, passphrase)
        .expect("encrypt");
    pem_encode(concat!("ENCRYPTED PRIVATE", " KEY"), encrypted.as_bytes())
}

#[tokio::test]
async fn client_certificates_authenticate_with_plain_and_encrypted_keys() {
    let pki = Pki::new();
    let (certificate, key) = pki.leaf("db.test", false);
    let server = server_config(certificate, &key, Some(&pki));
    let plain = TlsClient::new(&client_identity(&pki, None)).expect("plain key");
    handshake(&plain, Arc::clone(&server), "db.test")
        .await
        .expect("plain key handshake");
    let encrypted = TlsParams {
        key_passphrase: Some(SecretString::from("synthetic-passphrase")),
        ..client_identity(&pki, Some("synthetic-passphrase"))
    };
    let client = TlsClient::new(&encrypted).expect("decrypted key");
    handshake(&client, Arc::clone(&server), "db.test")
        .await
        .expect("encrypted key handshake");
    let anonymous = TlsClient::new(&pki.params(Verification::Full)).expect("client");
    let rejected = handshake(&anonymous, server, "db.test").await.err();
    assert!(
        rejected.is_some_and(|error| error.to_string().contains("CertificateRequired")),
        "a server requiring client certificates rejects anonymous clients"
    );
}

#[test]
fn encrypted_keys_need_the_right_passphrase() {
    let pki = Pki::new();
    let missing = client_identity(&pki, Some("synthetic-passphrase"));
    assert!(matches!(
        TlsClient::new(&missing),
        Err(TlsError::KeyPassphraseRequired { .. })
    ));
    let wrong = TlsParams {
        key_passphrase: Some(SecretString::from("not-it")),
        ..missing
    };
    assert!(matches!(
        TlsClient::new(&wrong),
        Err(TlsError::KeyPassphraseWrong { .. })
    ));
}

#[test]
fn client_identity_files_are_checked() {
    let pki = Pki::new();
    let half = TlsParams {
        client_key: None,
        ..client_identity(&pki, None)
    };
    assert!(matches!(
        TlsClient::new(&half),
        Err(TlsError::IncompleteClientIdentity)
    ));
    let legacy = TlsParams {
        client_key: Some(pki.write(
            "legacy.key",
            concat!(
                "-----BEGIN RSA PRIVATE",
                " KEY-----\nProc-Type: 4,ENCRYPTED\n",
                "DEK-Info: AES-128-CBC,00\n\nAAAA\n-----END RSA PRIVATE",
                " KEY-----\n",
            ),
        )),
        ..client_identity(&pki, None)
    };
    let error = TlsClient::new(&legacy).err().expect("rejected");
    assert!(error.to_string().contains("openssl pkcs8"), "{error}");
}

#[test]
fn params_debug_redacts_the_passphrase() {
    let params = TlsParams {
        key_passphrase: Some(SecretString::from("hunter2-synthetic")),
        ..TlsParams::new(Verification::Full)
    };
    let rendered = format!("{params:?}");
    assert!(!rendered.contains("hunter2"));
}
