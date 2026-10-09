// SPDX-License-Identifier: Apache-2.0
//! A saved connection as the command-line tools see it. Previews read the
//! profile only (no keychain, no tunnel); a job's [`Login`] resolves the
//! route like every connect (keychain or asked password, SSH tunnel with
//! prompts, proxy) and holds the 0600 password file, a bundled CA file and
//! the loopback forward until the job ends.
use std::path::PathBuf;

use openrows_driver_api::{ConnectRequest, Endpoint, SslMode};
use openrows_dump::{Address, ConnectionSpec, PrivateFile, TlsMode, TlsSpec};
use openrows_store::profile::{ConnectionProfile, PasswordStorage};

use super::dump_error;
use super::forward::Forward;
use crate::connections::route::profile_target;
use crate::connections::target::{self, DriverSecrets};
use crate::connections::{Connections, IpcError, IpcErrorKind};

/// Stands in for the password file in previews (always shown redacted).
const PREVIEW_PASSWORD_FILE: &str = "/openrows-preview/password";
/// Stands in for a bundled cloud CA file in previews.
const PREVIEW_CLOUD_CA: &str = "<bundled CA file>";
/// Stands in for the loopback port in previews; replaced by
/// [`TUNNEL_PORT_MARK`] for display.
pub(super) const PREVIEW_PORT: u16 = 0;
/// Shown for the loopback port, which is chosen when the job starts.
pub(super) const TUNNEL_PORT_MARK: &str = "<tunnel port>";

fn ssl_mode(mode: SslMode) -> TlsMode {
    match mode {
        SslMode::Disable => TlsMode::Disable,
        SslMode::Allow => TlsMode::Allow,
        SslMode::Prefer => TlsMode::Prefer,
        SslMode::Require => TlsMode::Require,
        SslMode::VerifyCa => TlsMode::VerifyCa,
        SslMode::VerifyFull => TlsMode::VerifyFull,
    }
}

/// The tools' view of `request`; `forwarded` is the loopback port when the
/// connection is routed through a tunnel or proxy.
fn spec_of(request: &ConnectRequest, forwarded: Option<u16>) -> Result<ConnectionSpec, IpcError> {
    let Endpoint::Server { host, port } = &request.endpoint else {
        return Err(IpcError::invalid(
            "File databases have no server to connect to",
        ));
    };
    let socket = host.starts_with('/');
    let address = match forwarded {
        Some(port) => Address::Forwarded {
            port,
            server_host: if socket { String::new() } else { host.clone() },
        },
        None if socket => Address::Socket(host.clone()),
        None => Address::Tcp {
            host: host.clone(),
            port: *port,
        },
    };
    let tls = &request.tls;
    let mode = if socket && forwarded.is_some() {
        // A socket on the SSH server: TLS does not apply.
        TlsMode::Disable
    } else {
        ssl_mode(request.ssl_mode)
    };
    Ok(ConnectionSpec {
        address,
        user: request.user.clone(),
        database: request.database.clone(),
        tls: TlsSpec {
            mode,
            ca_file: tls.ca_file.clone(),
            client_cert: tls.client_cert.clone(),
            client_key: tls.client_key.clone(),
        },
        application_name: format!("{} Backup", request.application_name),
        password_file: None,
    })
}

/// The login a preview shows, logged in to the profile's own database.
pub(super) fn preview(profile: &ConnectionProfile) -> Result<ConnectionSpec, IpcError> {
    let target = target::target(profile, &DriverSecrets::default())?;
    let routed = profile.ssh.is_some() || profile.proxy.is_some();
    let mut spec = spec_of(&target.request, routed.then_some(PREVIEW_PORT))?;
    let has_password =
        profile.secrets.password.is_some() || profile.password_storage == PasswordStorage::Ask;
    if has_password {
        spec.password_file = Some(PathBuf::from(PREVIEW_PASSWORD_FILE));
    }
    if spec.tls.ca_file.is_none() && profile.tls.cloud_ca.is_some() {
        spec.tls.ca_file = Some(PathBuf::from(PREVIEW_CLOUD_CA));
    }
    Ok(spec)
}

/// A live login; dropping it deletes its files and closes the forward.
pub(super) struct Login {
    pub(super) spec: ConnectionSpec,
    _password: Option<PrivateFile>,
    _ca: Option<PrivateFile>,
    _forward: Option<Forward>,
}

/// Log in to the profile's own database for a job of tools of the `MySQL`
/// family (`mysql`) or `PostgreSQL`, accepting up to `connections` tool
/// connections through a tunnel or proxy.
///
/// # Errors
/// Keychain, SSH or proxy failures, or TLS settings the tools cannot use.
pub(super) async fn login(
    connections: &Connections,
    profile_id: &str,
    mysql: bool,
    tool_connections: usize,
) -> Result<Login, IpcError> {
    let target = profile_target(connections, profile_id).await?;
    let request = &target.request;
    if request.tls.key_passphrase.is_some() {
        return Err(IpcError::new(
            IpcErrorKind::Unsupported,
            "The command-line tools cannot use an encrypted TLS client key",
        ));
    }
    let password = request
        .password
        .as_ref()
        .map(|secret| password_file(secret.expose(), mysql))
        .transpose()?;
    let ca = match (&request.tls.ca_file, request.tls.cloud_ca) {
        (None, Some(cloud)) => {
            Some(PrivateFile::new("ca", cloud.pem()).map_err(|e| dump_error(&e))?)
        }
        _ => None,
    };
    let forward = match &request.via {
        Some(dialer) => Some(
            Forward::open(std::sync::Arc::clone(dialer), tool_connections)
                .await
                .map_err(|error| IpcError::new(IpcErrorKind::Network, error.to_string()))?,
        ),
        None => None,
    };
    let mut spec = spec_of(request, forward.as_ref().map(Forward::port))?;
    spec.password_file = password.as_ref().map(|file| file.path().to_path_buf());
    if let Some(ca) = &ca {
        spec.tls.ca_file = Some(ca.path().to_path_buf());
    }
    Ok(Login {
        spec,
        _password: password,
        _ca: ca,
        _forward: forward,
    })
}

fn password_file(password: &str, mysql: bool) -> Result<PrivateFile, IpcError> {
    let file = if mysql {
        PrivateFile::mysql_options(password)
    } else {
        PrivateFile::pgpass(password)
    };
    file.map_err(|error| dump_error(&error))
}

#[cfg(test)]
mod tests {
    use openrows_store::profile::{Driver, EnvKind, EnvTag, ServerParams};

    use super::*;

    fn profile(host: &str) -> ConnectionProfile {
        let params = ServerParams {
            host: host.to_owned(),
            port: 5432,
            user: Some("app".to_owned()),
            database: Some("shop".to_owned()),
            ..ServerParams::default()
        };
        ConnectionProfile::new("p", Driver::Postgres, params, EnvTag::new(EnvKind::Dev))
    }

    #[test]
    fn previews_read_the_profile_without_secrets() -> Result<(), IpcError> {
        let spec = preview(&profile("db.example.com"))?;
        assert_eq!(
            spec.address,
            Address::Tcp {
                host: "db.example.com".to_owned(),
                port: 5432
            }
        );
        assert_eq!(
            (spec.user.as_str(), spec.database.as_str()),
            ("app", "shop")
        );
        assert_eq!(spec.password_file, None);
        let socket = preview(&profile("/var/run/postgresql"))?;
        assert_eq!(
            socket.address,
            Address::Socket("/var/run/postgresql".to_owned())
        );
        Ok(())
    }

    #[test]
    fn routed_connections_point_at_the_loopback_forward() -> Result<(), IpcError> {
        let request = target::target(&profile("db.internal"), &DriverSecrets::default())?.request;
        let spec = spec_of(&request, Some(40000))?;
        assert_eq!(
            spec.address,
            Address::Forwarded {
                port: 40000,
                server_host: "db.internal".to_owned()
            }
        );
        let socket = target::target(&profile("/tmp"), &DriverSecrets::default())?.request;
        let spec = spec_of(&socket, Some(40000))?;
        assert_eq!(spec.tls.mode, TlsMode::Disable);
        Ok(())
    }
}
