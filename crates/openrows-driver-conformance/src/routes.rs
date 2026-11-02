// SPDX-License-Identifier: Apache-2.0
//! [`crate::Group::Tls`] and [`crate::Group::Ssh`]: the transports a profile
//! can choose, against the certificates and the OpenSSH container that
//! `just test-pg` and `just test-mysql` provide (docs/17 §2.2).
//!
//! | Variable | Holds |
//! | --- | --- |
//! | `OPENROWS_TEST_TLS_DIR` | `ca.crt` (signed the server certificate, valid for `localhost` only) and `other-ca.crt` |
//! | `OPENROWS_TEST_SSH_PORT`, `OPENROWS_TEST_SSH_PASSWORD` | The published sshd and the password of its `tester` user |
//! | `OPENROWS_TEST_ROUTED_HOST`, `OPENROWS_TEST_ROUTED_PORT` | The database as the sshd sees it |

use std::path::PathBuf;
use std::sync::Arc;

use openrows_driver_api::{
    ConnectRequest, ConnectStage, ConnectTrace, DriverError, Endpoint, Feature, SslMode, TlsFiles,
    TlsStatus,
};
use openrows_net::known_hosts::KnownHosts;
use openrows_net::tls::{TlsDetails, Verification};
use openrows_net::{
    AuthMethod, BoxFuture, Challenge, Destination, FirstUseDecision, HostKeyPolicy,
    KeyboardInteractivePrompter, PromptCancelled, SecretString, SharedTunnel, SshHop, Timeouts,
    TunnelDialer, TunnelOptions, UnknownHost,
};

use crate::sink::ping;
use crate::streaming::{all_rows, cancel_contract};
use crate::{Failure, Target, ensure};

fn variable(name: &str) -> Result<String, Failure> {
    std::env::var(name).map_err(|_| {
        Failure::Setup(format!(
            "{name} is unset; run `just test-pg` or `just test-mysql`"
        ))
    })
}

fn tls_file(name: &str) -> Result<PathBuf, Failure> {
    Ok(PathBuf::from(variable("OPENROWS_TEST_TLS_DIR")?).join(name))
}

/// `request` for `host` (same port) in `mode`, trusting `ca` only.
fn with_tls(
    request: &ConnectRequest,
    host: &str,
    mode: SslMode,
    ca: &str,
) -> Result<ConnectRequest, Failure> {
    let Endpoint::Server { port, .. } = request.endpoint else {
        return Err(Failure::Setup("TLS needs a server endpoint".to_owned()));
    };
    let mut secured = request.clone();
    secured.endpoint = Endpoint::Server {
        host: host.to_owned(),
        port,
    };
    secured.ssl_mode = mode;
    secured.tls = TlsFiles {
        ca_file: Some(tls_file(ca)?),
        ..TlsFiles::default()
    };
    Ok(secured)
}

fn verified(tls: &TlsStatus) -> Result<&TlsDetails, Failure> {
    let TlsStatus::Encrypted(details) = tls else {
        return Err(Failure::Contract(format!("expected TLS, got {tls:?}")));
    };
    ensure(details.verification == Verification::Full, || {
        format!("verification {:?}", details.verification)
    })?;
    ensure(details.protocol.starts_with("TLS 1."), || {
        format!("protocol {}", details.protocol)
    })?;
    Ok(details)
}

pub(crate) async fn tls(target: &Target) -> Result<(), Failure> {
    let secured = with_tls(&target.request, "localhost", SslMode::VerifyFull, "ca.crt")?;
    let mut connection = target
        .driver
        .connect(&secured, &mut ConnectTrace::default())
        .await?;
    let details = verified(&connection.server().tls)?;
    ensure(
        details.subject.is_some() && details.issuer.is_some(),
        || {
            format!(
                "certificate {:?} from {:?}",
                details.subject, details.issuer
            )
        },
    )?;
    all_rows(connection.as_mut()).await?;
    connection.close().await?;
    let wrong_name = with_tls(&target.request, "127.0.0.1", SslMode::VerifyFull, "ca.crt")?;
    refused_at_tls(target, &wrong_name, "not valid for this host name").await?;
    let wrong_ca = with_tls(
        &target.request,
        "localhost",
        SslMode::VerifyFull,
        "other-ca.crt",
    )?;
    refused_at_tls(target, &wrong_ca, "not signed by a trusted CA").await?;
    plain(target).await
}

async fn refused_at_tls(
    target: &Target,
    request: &ConnectRequest,
    expected: &str,
) -> Result<(), Failure> {
    let mut trace = ConnectTrace::default();
    match target.driver.connect(request, &mut trace).await {
        Err(DriverError::Tls(message)) => {
            ensure(message.contains(expected), || {
                format!("TLS refusal {message:?}, expected {expected:?}")
            })?;
            ensure(trace.failed == Some(ConnectStage::Tls), || {
                format!("TLS refusal recorded at {:?}", trace.failed)
            })
        }
        Err(other) => Err(Failure::Contract(format!(
            "expected a TLS error, got {other}"
        ))),
        Ok(_) => Err(Failure::Contract(format!(
            "connected although {expected:?} should have refused it"
        ))),
    }
}

async fn plain(target: &Target) -> Result<(), Failure> {
    let mut request = target.request.clone();
    request.ssl_mode = SslMode::Disable;
    let mut connection = target
        .driver
        .connect(&request, &mut ConnectTrace::default())
        .await?;
    let tls = connection.server().tls.clone();
    ensure(tls == TlsStatus::Disabled, || {
        format!("sslmode=disable reported {tls:?}")
    })?;
    ping(connection.as_mut()).await?;
    connection.close().await?;
    Ok(())
}

/// The container's host key is generated per run, so it is trusted for this
/// tunnel only and nothing is written.
struct TrustThisRun;

impl HostKeyPolicy for TrustThisRun {
    fn on_unknown_host<'a>(&'a self, _host: &'a UnknownHost) -> BoxFuture<'a, FirstUseDecision> {
        Box::pin(async { FirstUseDecision::AcceptOnce })
    }
}

/// Password authentication never prompts.
struct NoPrompts;

impl KeyboardInteractivePrompter for NoPrompts {
    fn respond<'a>(
        &'a self,
        _challenge: &'a Challenge,
    ) -> BoxFuture<'a, Result<Vec<SecretString>, PromptCancelled>> {
        Box::pin(async { Err(PromptCancelled) })
    }
}

fn tunnel() -> Result<Arc<SharedTunnel>, Failure> {
    let port = variable("OPENROWS_TEST_SSH_PORT")?
        .parse()
        .map_err(|_| Failure::Setup("OPENROWS_TEST_SSH_PORT is not a port".to_owned()))?;
    let hop = SshHop {
        host: "127.0.0.1".to_owned(),
        port,
        user: "tester".to_owned(),
        auth: vec![AuthMethod::Password(SecretString::from(
            variable("OPENROWS_TEST_SSH_PASSWORD")?.as_str(),
        ))],
    };
    let options = TunnelOptions {
        known_hosts: Arc::new(KnownHosts::new(
            Vec::new(),
            std::env::temp_dir().join("orc-unused-known-hosts"),
        )),
        host_key_policy: Arc::new(TrustThisRun),
        prompter: Arc::new(NoPrompts),
        timeouts: Timeouts::default(),
        proxy: None,
    };
    Ok(Arc::new(SharedTunnel::new(vec![hop], options)))
}

fn routed_destination() -> Result<Destination, Failure> {
    Ok(Destination::Tcp {
        host: variable("OPENROWS_TEST_ROUTED_HOST")?,
        port: variable("OPENROWS_TEST_ROUTED_PORT")?
            .parse()
            .map_err(|_| Failure::Setup("OPENROWS_TEST_ROUTED_PORT is not a port".to_owned()))?,
    })
}

pub(crate) async fn ssh(target: &Target) -> Result<(), Failure> {
    let shared = tunnel()?;
    let route = Arc::new(TunnelDialer::new(
        Arc::clone(&shared),
        routed_destination()?,
    ));
    let routed = target.request.clone().through(route);
    let outcome = routed_contract(target, &routed).await;
    shared.close().await;
    outcome
}

async fn routed_contract(target: &Target, routed: &ConnectRequest) -> Result<(), Failure> {
    let mut connection = target
        .driver
        .connect(routed, &mut ConnectTrace::default())
        .await?;
    ping(connection.as_mut()).await?;
    all_rows(connection.as_mut()).await?;
    if target.driver.capabilities().supports(Feature::Cancel) {
        cancel_contract(connection.as_mut(), target.driver.dialect()).await?;
    }
    connection.close().await?;
    if !target.driver.capabilities().supports(Feature::Tls) {
        return Ok(());
    }
    let secured = with_tls(routed, "localhost", SslMode::VerifyFull, "ca.crt")?;
    let mut connection = target
        .driver
        .connect(&secured, &mut ConnectTrace::default())
        .await?;
    verified(&connection.server().tls)?;
    ping(connection.as_mut()).await?;
    connection.close().await?;
    Ok(())
}
