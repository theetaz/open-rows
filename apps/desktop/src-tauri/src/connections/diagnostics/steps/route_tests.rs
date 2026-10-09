// SPDX-License-Identifier: Apache-2.0
//! Steps for proxies, Unix sockets over SSH, certificate expiry and latency.
use std::io;

use openrows_driver_api::{ConnectStage, ConnectTrace, TlsStatus};
use openrows_net::tls::Verification;
use openrows_net::{ProxyError, SshError};

use super::super::super::error::{IpcError, IpcErrorKind};
use super::super::probe::LatencyDto;
use super::tests::{attempt, completed, details, run};
use super::{StepStatus, Steps, TestStepKind};

#[test]
fn the_first_ssh_hop_names_its_proxy_and_proxy_failures_are_connect_failures() {
    let mut steps = Steps {
        proxy: Some("SOCKS5 proxy proxy.corp:1080".to_owned()),
        ..Steps::default()
    };
    let refused = ProxyError::AuthFailed {
        proxy: "SOCKS5 proxy proxy.corp:1080".to_owned(),
    };
    let error = steps.ssh(&attempt(0, SshError::Proxy(refused)));
    assert!(error.is_some());
    let step = &steps.list[0];
    assert_eq!(step.kind, TestStepKind::SshConnect);
    assert_eq!(
        step.label,
        "Connect to SSH server jump.example.com:22 via SOCKS5 proxy proxy.corp:1080"
    );
    assert!(
        step.hint
            .as_deref()
            .is_some_and(|hint| hint.contains("Proxy section")),
        "{step:?}"
    );
}

#[test]
fn proxied_and_socket_routes_label_the_connect_step() {
    let trace = ConnectTrace {
        completed: completed(&[ConnectStage::Connect]),
        tls: Some(TlsStatus::LocalSocket),
        ..ConnectTrace::default()
    };
    let mut proxied = Steps::default();
    let via = super::DatabaseRun {
        proxy: Some("HTTP proxy proxy.corp:3128"),
        ..run(&trace, "db.example.com", false)
    };
    proxied.database(&via, None);
    assert_eq!(
        proxied.list[0].label,
        "Connect to db.example.com:5432 via HTTP proxy proxy.corp:3128"
    );
    let mut socket = Steps::default();
    let tunnelled = super::DatabaseRun {
        socket: Some("/var/run/postgresql/.s.PGSQL.5432"),
        ..run(&trace, "/var/run/postgresql", true)
    };
    socket.database(&tunnelled, None);
    assert_eq!(
        socket.list[0].label,
        "Open tunnel to /var/run/postgresql/.s.PGSQL.5432"
    );
    assert_eq!(socket.list[1].kind, TestStepKind::Tls);
    assert_eq!(socket.list[1].status, StepStatus::Ok);
}

#[test]
fn certificates_expiring_within_thirty_days_warn() {
    let mut expiring = details(Verification::Full);
    expiring.expires_in_days = Some(9);
    let trace = ConnectTrace {
        completed: completed(&[ConnectStage::Connect, ConnectStage::Tls]),
        tls: Some(TlsStatus::Encrypted(expiring)),
        ..ConnectTrace::default()
    };
    let mut steps = Steps::default();
    steps.database(&run(&trace, "db.example.com", false), None);
    let tls = &steps.list[1];
    assert_eq!(
        (tls.kind, tls.status),
        (TestStepKind::Tls, StepStatus::Warning)
    );
    assert!(
        tls.detail
            .as_deref()
            .is_some_and(|detail| detail.starts_with("Certificate expires in 9 days · TLS 1.3")),
        "{tls:?}"
    );
}

#[test]
fn latency_is_a_timed_step_and_a_failed_probe_only_warns() {
    let mut steps = Steps::default();
    steps.latency(&Ok(LatencyDto {
        median_ms: 2.4,
        min_ms: 1.9,
        max_ms: 3.1,
        samples: 3,
    }));
    let step = &steps.list[0];
    assert_eq!(
        (step.kind, step.status),
        (TestStepKind::Latency, StepStatus::Ok)
    );
    assert_eq!(step.elapsed_ms, Some(2));
    assert_eq!(
        step.detail.as_deref(),
        Some("median 2.4 ms · min 1.9 ms · max 3.1 ms over 3 queries")
    );
    let error = IpcError::new(IpcErrorKind::Network, io::Error::other("reset").to_string());
    steps.latency(&Err(error));
    assert_eq!(steps.list[1].status, StepStatus::Warning);
}
