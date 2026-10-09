// SPDX-License-Identifier: Apache-2.0
//! What to change when a Test connection step fails (M1-07: every failing
//! step gets a specific, actionable message). TLS and host-key errors carry
//! their advice in the message already, so they get no extra hint.
use openrows_store::profile::Driver as Engine;

use super::super::error::{IpcError, IpcErrorKind};
use super::steps::TestStepKind;

/// SQLSTATE `invalid_catalog_name`: the database does not exist.
const NO_DATABASE: &str = "3D000";

/// The hint for a failed `step` on `engine`, or `None` when the message says it all.
pub(crate) fn hint(step: TestStepKind, error: &IpcError, engine: Engine) -> Option<&'static str> {
    if matches!(error.kind, IpcErrorKind::Cancelled | IpcErrorKind::HostKey) {
        return None;
    }
    if error.message.contains(" proxy ") {
        return Some(
            "Check the proxy type, host, port, user and password in the Proxy section, and \
             that the proxy allows connections to this host and port.",
        );
    }
    match step {
        TestStepKind::SshConnect => Some(
            "Check the SSH host and port, and that the SSH server is running and reachable \
             from this Mac (VPN, firewall).",
        ),
        TestStepKind::SshAuth => Some(
            "Check the SSH user and authentication method. A key file needs its passphrase; \
             SSH agent needs the key loaded (ssh-add -l).",
        ),
        TestStepKind::Tunnel => Some(
            "The SSH server could not reach the database. Enter the database host as the SSH \
             server sees it (often localhost) and check that sshd allows TCP forwarding.",
        ),
        TestStepKind::Dns => Some(
            "Check the host name. Hosts that only resolve inside a private network need a \
             connection over SSH.",
        ),
        TestStepKind::Connect => Some(connect_hint(error, engine)),
        TestStepKind::Tls | TestStepKind::Version | TestStepKind::Latency => None,
        TestStepKind::Authenticate if engine == Engine::Postgres => postgres_login_hint(error),
        TestStepKind::Authenticate => mysql_login_hint(error),
    }
}

fn connect_hint(error: &IpcError, engine: Engine) -> &'static str {
    if error.message.contains("timed out") {
        return "Nothing answered in time: check the host, the port and any firewall or VPN \
                between this Mac and the server.";
    }
    match engine {
        Engine::Postgres => {
            "Check that PostgreSQL is running and listens on this host and port \
             (listen_addresses, port), and that no firewall blocks it."
        }
        Engine::Mysql | Engine::Mariadb => {
            "Check that the server is running and listens on this host and port \
             (bind-address, port), and that no firewall blocks it."
        }
        Engine::Sqlite => {
            "Check that the file exists and that OpenRows may read it (macOS Files and \
             Folders privacy settings), or choose New File to create one."
        }
    }
}

fn mysql_login_hint(error: &IpcError) -> Option<&'static str> {
    match error.kind {
        IpcErrorKind::Auth => Some(
            "Check the user and password. The account must be allowed from this host \
             ('user'@'host'); over SSH that is the SSH server's address.",
        ),
        IpcErrorKind::Server if error.message.contains("Unknown database") => {
            Some("The database does not exist: check the Database field.")
        }
        IpcErrorKind::Server => Some("The server refused the session; its message says why."),
        _ => None,
    }
}

fn postgres_login_hint(error: &IpcError) -> Option<&'static str> {
    match error.kind {
        IpcErrorKind::Auth => Some(
            "Check the user and password. The server's pg_hba.conf must allow this user and \
             database from this address.",
        ),
        IpcErrorKind::Server if error.code.as_deref() == Some(NO_DATABASE) => {
            Some("The database does not exist: check the Database field.")
        }
        IpcErrorKind::Server => Some(
            "The server refused the session; its message says why (pg_hba.conf, connection \
             limits or a role setting).",
        ),
        IpcErrorKind::Network if error.message.contains("timed out") => Some(
            "The server accepted the connection but did not finish logging in; check server \
             load and pg_hba.conf authentication methods.",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn error(kind: IpcErrorKind, message: &str) -> IpcError {
        IpcError::new(kind, message)
    }

    #[test]
    fn every_reachability_failure_gets_a_hint() {
        let refused = error(IpcErrorKind::Network, "127.0.0.1:5432: Connection refused");
        let timed_out = error(IpcErrorKind::Network, "TCP connect timed out after 10s");
        let connect = hint(TestStepKind::Connect, &refused, Engine::Postgres).unwrap_or_default();
        assert!(connect.contains("listen_addresses"));
        let slow = hint(TestStepKind::Connect, &timed_out, Engine::Postgres).unwrap_or_default();
        assert!(slow.contains("firewall or VPN"));
        for step in [
            TestStepKind::SshConnect,
            TestStepKind::SshAuth,
            TestStepKind::Tunnel,
            TestStepKind::Dns,
        ] {
            assert!(hint(step, &refused, Engine::Mysql).is_some(), "{step:?}");
        }
    }

    #[test]
    fn authentication_hints_follow_the_error() {
        let password = error(IpcErrorKind::Auth, "password authentication failed");
        let auth =
            hint(TestStepKind::Authenticate, &password, Engine::Postgres).unwrap_or_default();
        assert!(auth.contains("pg_hba.conf"));
        let missing = IpcError {
            code: Some(NO_DATABASE.to_owned()),
            ..error(IpcErrorKind::Server, "database \"x\" does not exist")
        };
        let database =
            hint(TestStepKind::Authenticate, &missing, Engine::Postgres).unwrap_or_default();
        assert!(database.contains("Database field"));
        let mysql = error(IpcErrorKind::Auth, "Access denied for user");
        let login = hint(TestStepKind::Authenticate, &mysql, Engine::Mariadb).unwrap_or_default();
        assert!(login.contains("'user'@'host'") && !login.contains("pg_hba"));
        let file = hint(TestStepKind::Connect, &mysql, Engine::Sqlite).unwrap_or_default();
        assert!(file.contains("New File"));
    }

    #[test]
    fn host_key_stops_cancellations_and_tls_carry_their_own_advice() {
        let changed = error(IpcErrorKind::HostKey, "The SSH host key has changed");
        assert_eq!(
            hint(TestStepKind::SshAuth, &changed, Engine::Postgres),
            None
        );
        let cancelled = error(IpcErrorKind::Cancelled, "cancelled");
        assert_eq!(
            hint(TestStepKind::SshAuth, &cancelled, Engine::Postgres),
            None
        );
        let tls = error(IpcErrorKind::Tls, "choose its CA certificate file");
        assert_eq!(hint(TestStepKind::Tls, &tls, Engine::Postgres), None);
    }
}
