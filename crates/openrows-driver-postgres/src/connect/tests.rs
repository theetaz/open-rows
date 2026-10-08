// SPDX-License-Identifier: Apache-2.0
use super::*;

#[test]
fn socket_directories_become_unix_targets() {
    let Target::Unix(path) = Target::of("/tmp", 5433, None) else {
        unreachable!("a leading slash means a socket directory")
    };
    assert_eq!(path, PathBuf::from("/tmp/.s.PGSQL.5433"));
    assert!(matches!(
        Target::of("localhost", 5432, None),
        Target::Tcp { port: 5432, .. }
    ));
}

#[test]
fn allow_retries_with_tls_only_after_a_rejection() {
    let rejected = DriverError::Auth("no pg_hba.conf entry".to_owned());
    assert!(retry_with_tls(
        SslMode::Allow,
        Negotiation::Plaintext,
        &rejected
    ));
    assert!(!retry_with_tls(
        SslMode::Prefer,
        Negotiation::Preferred,
        &rejected
    ));
    let refused = DriverError::Network(io::ErrorKind::ConnectionRefused.into());
    assert!(!retry_with_tls(
        SslMode::Allow,
        Negotiation::Plaintext,
        &refused
    ));
}

#[tokio::test]
async fn a_stage_timeout_names_the_stage() {
    let never = std::future::pending::<Result<(), DriverError>>();
    let error = within(Duration::from_millis(1), "TCP connect", never).await;
    let Err(DriverError::Network(error)) = error else {
        unreachable!("pending future must time out")
    };
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(error.to_string(), "TCP connect timed out after 1ms");
}
