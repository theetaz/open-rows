// SPDX-License-Identifier: Apache-2.0
//! Owned connect configuration; no trait objects or borrowed keychain values.
use openrows_driver_api::{
    Attachment, ConnectRequest, ConnectTimeouts, DriverError, Endpoint, FileSource, SslMode,
    TlsFiles,
};
use openrows_net::{SecretString, tls::CloudCa};
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path, time::Duration};
use zeroize::Zeroize;

/// Zeroizable wire secret with fixed redacted diagnostics.
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireSecret(String);
impl WireSecret {
    /// Take a secret copy for authenticated transport only.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }
    /// Borrow for authentication or driver conversion.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for WireSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WireSecret([redacted])")
    }
}
impl Drop for WireSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// File identity; memory names are host-local and non-durable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WireFile {
    /// Disk file, without lossy path conversion.
    Disk {
        /// UTF-8 disk path.
        path: String,
        /// Refuse writes.
        read_only: bool,
        /// Explicit create permission.
        create: bool,
    },
    /// Host-local memory database.
    Memory {
        /// Validated driver name.
        name: String,
    },
}
/// Serializable endpoint and ordered attachments.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WireEndpoint {
    /// Server location (the current registry refuses it).
    Server {
        /// Host identity.
        host: String,
        /// Port.
        port: u16,
    },
    /// File location.
    File {
        /// Main source.
        source: WireFile,
        /// Alias/path pairs.
        #[serde(deserialize_with = "crate::bounded::list")]
        attached: Vec<(String, String)>,
    },
}
/// TLS file references and zeroizable key passphrase.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireTls {
    /// Explicit CA path.
    pub ca: Option<String>,
    /// Cloud set: 1 AWS, 2 Azure.
    pub cloud: Option<u8>,
    /// Client certificate.
    pub certificate: Option<String>,
    /// Client key.
    pub key: Option<String>,
    /// Key passphrase.
    pub passphrase: Option<WireSecret>,
}
/// Complete owned connect payload, sent only after exact handshake agreement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OwnedConnect {
    /// Server or local file.
    pub endpoint: WireEndpoint,
    /// Database name.
    pub database: String,
    /// Login user.
    pub user: String,
    /// Zeroizable password.
    pub password: Option<WireSecret>,
    /// Application name.
    pub application: String,
    /// Explicit SSL mode ID, 0..=5 in documented mode order.
    pub ssl_mode: u8,
    /// TLS configuration.
    pub tls: WireTls,
    /// DNS, TCP/file open, TLS, authentication timeouts in milliseconds.
    pub timeouts_ms: [u64; 4],
    /// Ordered startup SQL.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub startup: Vec<String>,
    /// Non-secret option pairs.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub options: Vec<(String, String)>,
}
impl TryFrom<&ConnectRequest> for OwnedConnect {
    type Error = DriverError;
    fn try_from(request: &ConnectRequest) -> Result<Self, Self::Error> {
        if request.via.is_some() {
            return Err(unsupported("hosted routes"));
        }
        let timeout = request.timeouts;
        let timeouts_ms = [timeout.dns, timeout.tcp, timeout.tls, timeout.auth]
            .map(|value| u64::try_from(value.as_millis()).unwrap_or(u64::MAX));
        Ok(Self {
            endpoint: endpoint(&request.endpoint)?,
            database: request.database.clone(),
            user: request.user.clone(),
            password: secret(request.password.as_ref()),
            application: request.application_name.clone(),
            ssl_mode: ssl_id(request.ssl_mode),
            tls: tls(&request.tls)?,
            timeouts_ms,
            startup: request.startup_commands.clone(),
            options: request
                .driver_options
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        })
    }
}
impl OwnedConnect {
    /// Convert to the driver API inside the authenticated host worker.
    /// # Errors
    /// Invalid mode/cloud IDs or unsupported path/route configuration.
    pub fn to_request(&self) -> Result<ConnectRequest, DriverError> {
        let [dns, tcp, tls, auth] = self.timeouts_ms.map(Duration::from_millis);
        let modes = [
            SslMode::Disable,
            SslMode::Allow,
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
            SslMode::VerifyFull,
        ];
        let ssl_mode = *modes
            .get(usize::from(self.ssl_mode))
            .ok_or_else(|| unsupported("SSL mode"))?;
        Ok(ConnectRequest {
            endpoint: self.endpoint.to_endpoint(),
            database: self.database.clone(),
            user: self.user.clone(),
            password: self
                .password
                .as_ref()
                .map(|value| SecretString::from(value.expose())),
            application_name: self.application.clone(),
            ssl_mode,
            tls: self.tls.to_files()?,
            via: None,
            timeouts: ConnectTimeouts {
                dns,
                tcp,
                tls,
                auth,
            },
            startup_commands: self.startup.clone(),
            driver_options: self.options.iter().cloned().collect(),
        })
    }
}
impl WireEndpoint {
    fn to_endpoint(&self) -> Endpoint {
        match self {
            Self::Server { host, port } => Endpoint::Server {
                host: host.clone(),
                port: *port,
            },
            Self::File { source, attached } => Endpoint::File {
                source: match source {
                    WireFile::Disk {
                        path,
                        read_only,
                        create,
                    } => FileSource::Disk {
                        path: path.into(),
                        read_only: *read_only,
                        create: *create,
                    },
                    WireFile::Memory { name } => FileSource::Memory { name: name.clone() },
                },
                attached: attached
                    .iter()
                    .map(|(alias, path)| Attachment {
                        alias: alias.clone(),
                        path: path.into(),
                    })
                    .collect(),
            },
        }
    }
}
impl WireTls {
    fn to_files(&self) -> Result<TlsFiles, DriverError> {
        let cloud_ca = match self.cloud {
            None => None,
            Some(1) => Some(CloudCa::AwsRds),
            Some(2) => Some(CloudCa::Azure),
            Some(_) => return Err(unsupported("cloud CA")),
        };
        Ok(TlsFiles {
            ca_file: self.ca.as_ref().map(Into::into),
            cloud_ca,
            client_cert: self.certificate.as_ref().map(Into::into),
            client_key: self.key.as_ref().map(Into::into),
            key_passphrase: self
                .passphrase
                .as_ref()
                .map(|value| SecretString::from(value.expose())),
        })
    }
}
fn endpoint(value: &Endpoint) -> Result<WireEndpoint, DriverError> {
    Ok(match value {
        Endpoint::Server { host, port } => WireEndpoint::Server {
            host: host.clone(),
            port: *port,
        },
        Endpoint::File { source, attached } => WireEndpoint::File {
            source: match source {
                FileSource::Disk {
                    path: value,
                    read_only,
                    create,
                } => WireFile::Disk {
                    path: path(value)?,
                    read_only: *read_only,
                    create: *create,
                },
                FileSource::Memory { name } => WireFile::Memory { name: name.clone() },
            },
            attached: attached
                .iter()
                .map(|value| Ok((value.alias.clone(), path(&value.path)?)))
                .collect::<Result<_, DriverError>>()?,
        },
    })
}
fn tls(value: &TlsFiles) -> Result<WireTls, DriverError> {
    Ok(WireTls {
        ca: value.ca_file.as_deref().map(path).transpose()?,
        cloud: value.cloud_ca.map(|set| match set {
            CloudCa::AwsRds => 1,
            CloudCa::Azure => 2,
        }),
        certificate: value.client_cert.as_deref().map(path).transpose()?,
        key: value.client_key.as_deref().map(path).transpose()?,
        passphrase: secret(value.key_passphrase.as_ref()),
    })
}
fn secret(value: Option<&SecretString>) -> Option<WireSecret> {
    value.map(|value| WireSecret::new(value.expose().to_owned()))
}
fn path(value: &Path) -> Result<String, DriverError> {
    value
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| unsupported("non-UTF8 host path"))
}
fn unsupported(value: &str) -> DriverError {
    DriverError::Unsupported(value.to_owned())
}
fn ssl_id(value: SslMode) -> u8 {
    match value {
        SslMode::Disable => 0,
        SslMode::Allow => 1,
        SslMode::Prefer => 2,
        SslMode::Require => 3,
        SslMode::VerifyCa => 4,
        SslMode::VerifyFull => 5,
    }
}
