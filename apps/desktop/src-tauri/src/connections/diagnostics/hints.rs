// SPDX-License-Identifier: Apache-2.0
//! What to change when a Test connection step fails (M1-07: every failing
//! step gets a specific, actionable message). TLS and host-key errors carry
//! their advice in the message already, so they get no extra hint.
use super::super::error::{IpcError, IpcErrorKind};
use super::steps::TestStepKind;

/// SQLSTATE `invalid_catalog_name`: the database does not exist.
const NO_DATABASE: &str = "3D000";

/// The hint for a failed `step`, or `None` when the message says it all.
pub(crate) fn hint(step: TestStepKind, error: &IpcError) -> Option<&'static str> {
    if matches!(error.kind, IpcErrorKind::Cancelled | IpcErrorKind::HostKey) {
        return None;
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
        TestStepKind::Connect => Some(connect_hint(error)),
        TestStepKind::Tls | TestStepKind::Version => None,
        TestStepKind::Authenticate => authenticate_hint(error),
    }
}

fn connect_hint(error: &IpcError) -> &'static str {
    if error.message.contains("timed out") {
        "Nothing answered in time: check the host, the port and any firewall or VPN between \
         this Mac and the server."
    } else {
        "Check that PostgreSQL is running and listens on this host and port \
         (listen_addresses, port), and that no firewall blocks it."
    }
}

fn authenticate_hint(error: &IpcError) -> Option<&'static str> {
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
        let connect = hint(TestStepKind::Connect, &refused).unwrap_or_default();
        assert!(connect.contains("listen_addresses"));
        let slow = hint(TestStepKind::Connect, &timed_out).unwrap_or_default();
        assert!(slow.contains("firewall or VPN"));
        for step in [
            TestStepKind::SshConnect,
            TestStepKind::SshAuth,
            TestStepKind::Tunnel,
            TestStepKind::Dns,
        ] {
            assert!(hint(step, &refused).is_some(), "{step:?}");
        }
    }

    #[test]
    fn authentication_hints_follow_the_error() {
        let password = error(IpcErrorKind::Auth, "password authentication failed");
        let auth = hint(TestStepKind::Authenticate, &password).unwrap_or_default();
        assert!(auth.contains("pg_hba.conf"));
        let missing = IpcError {
            code: Some(NO_DATABASE.to_owned()),
            ..error(IpcErrorKind::Server, "database \"x\" does not exist")
        };
        let database = hint(TestStepKind::Authenticate, &missing).unwrap_or_default();
        assert!(database.contains("Database field"));
    }

    #[test]
    fn host_key_stops_cancellations_and_tls_carry_their_own_advice() {
        let changed = error(IpcErrorKind::HostKey, "The SSH host key has changed");
        assert_eq!(hint(TestStepKind::SshAuth, &changed), None);
        let cancelled = error(IpcErrorKind::Cancelled, "cancelled");
        assert_eq!(hint(TestStepKind::SshAuth, &cancelled), None);
        let tls = error(IpcErrorKind::Tls, "choose its CA certificate file");
        assert_eq!(hint(TestStepKind::Tls, &tls), None);
    }
}
