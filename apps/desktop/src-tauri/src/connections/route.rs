// SPDX-License-Identifier: Apache-2.0
//! The route step of every connect (D-10, D-11, M1-06): keychain secrets,
//! TLS files, an optional proxy and, for profiles with SSH, the profile's
//! shared tunnel. The result is a driver [`Target`] that says where to
//! connect (direct, through the proxy, or a stream through the tunnel to a
//! TCP port or a Unix socket on the SSH server) and with which TLS
//! parameters. Engine-agnostic: the profile's driver is resolved in
//! [`super::target`].
use std::sync::Arc;

use openrows_net::{Destination, ProxyDialer, ProxySpec, TunnelDialer};
use openrows_store::profile::{
    ConnectionProfile, Driver, PasswordStorage, ProfileService, SecretSlot,
};

use super::Connections;
use super::error::IpcError;
use super::profiles::profile_id;
use super::proxy_dto::proxy_spec;
use super::target::{self, DriverSecrets, Target};
use super::tunnels::{SshSecrets, route_spec};

/// libpq's socket file name prefix inside a socket directory.
const PG_SOCKET_PREFIX: &str = ".s.PGSQL.";

/// Driver secrets of a saved profile; the keychain is read only for secrets
/// the profile references.
///
/// # Errors
/// `Secret` when the keychain cannot be read.
pub(crate) fn driver_secrets(
    service: &ProfileService,
    profile: &ConnectionProfile,
) -> Result<DriverSecrets, IpcError> {
    Ok(DriverSecrets {
        password: service.secret(profile, SecretSlot::Password)?,
        tls_key_passphrase: service.secret(profile, SecretSlot::TlsKeyPassphrase)?,
    })
}

/// SSH secrets of a saved profile.
///
/// # Errors
/// `Secret` when the keychain cannot be read.
pub(crate) fn ssh_secrets(
    service: &ProfileService,
    profile: &ConnectionProfile,
) -> Result<SshSecrets, IpcError> {
    Ok(SshSecrets {
        password: service.secret(profile, SecretSlot::SshPassword)?,
        passphrase: service.secret(profile, SecretSlot::SshPassphrase)?,
    })
}

/// The profile's proxy with its keychain password, if it has one.
///
/// # Errors
/// `Secret` when the keychain cannot be read.
pub(crate) fn profile_proxy(
    service: &ProfileService,
    profile: &ConnectionProfile,
) -> Result<Option<Arc<ProxySpec>>, IpcError> {
    let Some(settings) = &profile.proxy else {
        return Ok(None);
    };
    let password = service.secret(profile, SecretSlot::ProxyPassword)?;
    Ok(Some(proxy_spec(settings, password.as_ref())))
}

/// What the tunnel connects to on the SSH server: the database's TCP port,
/// or its Unix socket when the host is a path (libpq: a socket directory;
/// `MySQL`: the socket file).
pub(crate) fn destination(profile: &ConnectionProfile) -> Destination {
    let params = &profile.params;
    if !params.host.starts_with('/') {
        return Destination::Tcp {
            host: params.host.clone(),
            port: params.port,
        };
    }
    let path = match profile.driver {
        Driver::Postgres => pg_socket(&params.host, params.port),
        Driver::Mysql
        | Driver::Mariadb
        | Driver::Sqlserver
        | Driver::Clickhouse
        | Driver::Sqlite
        | Driver::Libsql
        | Driver::LibsqlFile => params.host.clone(),
    };
    Destination::Socket(path)
}

/// `/var/run/postgresql` → `/var/run/postgresql/.s.PGSQL.5432`; a full
/// socket path is kept.
fn pg_socket(host: &str, port: u16) -> String {
    let is_file = host
        .rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with(PG_SOCKET_PREFIX));
    if is_file {
        return host.to_owned();
    }
    format!("{}/{PG_SOCKET_PREFIX}{port}", host.trim_end_matches('/'))
}

/// `target` through `proxy` straight to the database (no SSH).
pub(crate) fn through_proxy(
    target: Target,
    proxy: Arc<ProxySpec>,
    profile: &ConnectionProfile,
) -> Target {
    let dialer = ProxyDialer::new(proxy, &profile.params.host, profile.params.port);
    target.through(Arc::new(dialer))
}

/// Where and how to reach saved connection `id`: its driver target, through
/// its proxy and/or the profile's shared SSH tunnel (opened on first use,
/// with prompts; reopened when it dropped). An "ask every time" profile uses
/// the password typed on connect.
///
/// # Errors
/// Profile, keychain, SSH (typed: `ssh`, `hostKey`, `cancelled`), a missing
/// asked password (`auth`) or validation failures.
pub(crate) async fn profile_target(state: &Connections, id: &str) -> Result<Target, IpcError> {
    let profile = state.profiles.get(profile_id(id)?)?;
    let mut secrets = driver_secrets(&state.profiles, &profile)?;
    secrets.password = match profile.password_storage {
        PasswordStorage::Keychain => secrets.password,
        PasswordStorage::Ask => Some(state.asked.password(&profile)?),
        PasswordStorage::None => None,
        PasswordStorage::Command => {
            Some(super::credentials::password(state, &state.app, &profile).await?)
        }
    };
    let target = target::target(&profile, &secrets)?;
    if profile.helper.is_some() {
        let helper = super::helpers::acquire(state, &profile).await?;
        return Ok(target.through(Arc::new(openrows_net::helper::HelperDialer::new(helper))));
    }
    let proxy = profile_proxy(&state.profiles, &profile)?;
    let Some(ssh) = &profile.ssh else {
        return Ok(match proxy {
            Some(proxy) => through_proxy(target, proxy, &profile),
            None => target,
        });
    };
    let plan = async {
        let spec = route_spec(ssh, ssh_secrets(&state.profiles, &profile)?)?;
        state.opener.shared(&spec, proxy).await
    };
    let shared = state.tunnels.get_or_open(id, Box::pin(plan)).await?;
    let dialer = TunnelDialer::new(shared, destination(&profile));
    Ok(target.through(Arc::new(dialer)))
}

#[cfg(test)]
mod tests {
    use openrows_store::profile::{EnvKind, EnvTag, ServerParams};

    use super::*;

    fn profile(driver: Driver, host: &str, port: u16) -> ConnectionProfile {
        let params = ServerParams {
            host: host.to_owned(),
            port,
            ..ServerParams::default()
        };
        ConnectionProfile::new("p", driver, params, EnvTag::new(EnvKind::Dev))
    }

    #[test]
    fn socket_hosts_become_socket_destinations_on_the_ssh_server() {
        let socket = |driver, host| destination(&profile(driver, host, 5432));
        assert_eq!(
            socket(Driver::Postgres, "/var/run/postgresql/"),
            Destination::Socket("/var/run/postgresql/.s.PGSQL.5432".to_owned())
        );
        assert_eq!(
            socket(Driver::Postgres, "/var/run/postgresql/.s.PGSQL.6432"),
            Destination::Socket("/var/run/postgresql/.s.PGSQL.6432".to_owned())
        );
        assert_eq!(
            socket(Driver::Mysql, "/var/run/mysqld/mysqld.sock"),
            Destination::Socket("/var/run/mysqld/mysqld.sock".to_owned())
        );
        assert_eq!(
            socket(Driver::Mariadb, "db.internal"),
            Destination::Tcp {
                host: "db.internal".to_owned(),
                port: 5432
            }
        );
    }
}
