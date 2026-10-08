// SPDX-License-Identifier: Apache-2.0
use std::io;
use std::path::PathBuf;

use openrows_net::known_hosts::KnownHostsLine;
use openrows_net::{AuthMethod, HopTiming, SshHop};

use super::super::super::error::IpcErrorKind;
use super::super::super::tunnels::TunnelAttempt;
use super::*;

fn hop(host: &str, user: &str) -> SshHop {
    SshHop {
        host: host.to_owned(),
        port: 22,
        user: user.to_owned(),
        auth: vec![AuthMethod::KeyboardInteractive],
    }
}

fn timing(host: &str) -> HopTiming {
    HopTiming {
        host: host.to_owned(),
        connect: Duration::from_millis(12),
        handshake: Duration::from_millis(40),
    }
}

fn attempt(completed: usize, error: SshError) -> TunnelAttempt {
    let hops = vec![
        hop("jump.example.com", "ops"),
        hop("bastion.internal", "app"),
    ];
    TunnelAttempt {
        completed: hops
            .iter()
            .take(completed)
            .map(|hop| timing(&hop.host))
            .collect(),
        hops,
        result: Err(error),
    }
}

fn summary(steps: &Steps) -> Vec<(TestStepKind, StepStatus)> {
    steps
        .steps
        .iter()
        .map(|step| (step.kind, step.status))
        .collect()
}

#[test]
fn ssh_steps_show_completed_hops_and_the_failing_login() {
    let mut steps = Steps::default();
    let error = steps.ssh(&attempt(
        1,
        SshError::AuthFailed {
            user: "app".to_owned(),
            host: "bastion.internal".to_owned(),
            tried: "keyboard-interactive".to_owned(),
        },
    ));
    assert_eq!(error.map(|error| error.kind), Some(IpcErrorKind::Ssh));
    assert_eq!(
        summary(&steps),
        [
            (TestStepKind::SshConnect, StepStatus::Ok),
            (TestStepKind::SshAuth, StepStatus::Ok),
            (TestStepKind::SshAuth, StepStatus::Failed),
        ]
    );
    assert_eq!(
        steps.steps[0].label,
        "Connect to SSH server jump.example.com:22"
    );
    assert_eq!(steps.steps[0].elapsed_ms, Some(12));
    assert_eq!(steps.steps[2].label, "SSH login as app@bastion.internal");
}

#[test]
fn unreachable_ssh_servers_fail_the_connect_step() {
    let mut steps = Steps::default();
    let refused = SshError::Connect {
        host: "jump.example.com".to_owned(),
        port: 22,
        source: io::ErrorKind::ConnectionRefused.into(),
    };
    assert!(steps.ssh(&attempt(0, refused)).is_some());
    assert_eq!(
        summary(&steps),
        [(TestStepKind::SshConnect, StepStatus::Failed)]
    );
    let mut next = Steps::default();
    let timeout = SshError::Timeout {
        stage: Stage::ChannelOpen,
        after: Duration::from_secs(10),
    };
    next.ssh(&attempt(1, timeout));
    assert_eq!(next.steps[2].kind, TestStepKind::SshConnect);
    assert_eq!(next.steps[2].label, "Reach jump target bastion.internal:22");
}

#[test]
fn a_changed_host_key_fails_the_login_step_as_a_hard_stop() {
    let mut steps = Steps::default();
    let changed = SshError::HostKeyChanged {
        host: "jump.example.com".to_owned(),
        fingerprint: "SHA256:new".to_owned(),
        at: KnownHostsLine {
            path: PathBuf::from("/home/me/.ssh/known_hosts"),
            line: 7,
        },
    };
    let error = steps.ssh(&attempt(0, changed));
    assert_eq!(error.map(|error| error.kind), Some(IpcErrorKind::HostKey));
    assert_eq!(
        summary(&steps),
        [(TestStepKind::SshAuth, StepStatus::Failed)]
    );
    assert!(
        steps.steps[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("has changed"))
    );
}

fn run<'a>(trace: &'a ConnectTrace, host: &'a str, tunnelled: bool) -> DatabaseRun<'a> {
    DatabaseRun {
        trace,
        host,
        port: 5432,
        user: "app",
        tunnelled,
        file: false,
        server: None,
    }
}

fn details(verification: Verification) -> TlsDetails {
    TlsDetails {
        protocol: "TLS 1.3".to_owned(),
        cipher: "TLS13_AES_256_GCM_SHA384".to_owned(),
        subject: Some("CN=db.example.com".to_owned()),
        issuer: None,
        not_after: None,
        verification,
    }
}

fn completed(stages: &[ConnectStage]) -> Vec<(ConnectStage, Duration)> {
    stages
        .iter()
        .map(|stage| (*stage, Duration::from_millis(3)))
        .collect()
}

#[test]
fn unverified_tls_to_a_remote_host_is_a_warning_with_details() {
    let trace = ConnectTrace {
        completed: completed(&[
            ConnectStage::Connect,
            ConnectStage::Tls,
            ConnectStage::Authenticate,
        ]),
        tls: Some(TlsStatus::Encrypted(details(Verification::None))),
        ..ConnectTrace::default()
    };
    let mut steps = Steps::default();
    steps.database(&run(&trace, "db.example.com", false), None);
    assert_eq!(
        summary(&steps),
        [
            (TestStepKind::Connect, StepStatus::Ok),
            (TestStepKind::Tls, StepStatus::Warning),
            (TestStepKind::Authenticate, StepStatus::Ok),
        ]
    );
    let detail = steps.steps[1].detail.clone().unwrap_or_default();
    assert!(detail.starts_with("TLS 1.3 · TLS13_AES_256_GCM_SHA384 · CN=db.example.com"));
    assert!(detail.ends_with("certificate NOT verified"));

    let verified = ConnectTrace {
        tls: Some(TlsStatus::Encrypted(details(Verification::Full))),
        ..trace
    };
    let mut steps = Steps::default();
    steps.database(&run(&verified, "db.example.com", false), None);
    assert_eq!(steps.steps[1].status, StepStatus::Ok);
}

#[test]
fn tunnelled_plaintext_is_fine_but_direct_plaintext_warns() {
    let trace = ConnectTrace {
        completed: completed(&[ConnectStage::Connect, ConnectStage::Authenticate]),
        tls: Some(TlsStatus::Disabled),
        ..ConnectTrace::default()
    };
    let mut tunnelled = Steps::default();
    tunnelled.database(&run(&trace, "10.0.0.5", true), None);
    assert_eq!(
        summary(&tunnelled),
        [
            (TestStepKind::Tunnel, StepStatus::Ok),
            (TestStepKind::Tls, StepStatus::Ok),
            (TestStepKind::Authenticate, StepStatus::Ok),
        ]
    );
    assert_eq!(tunnelled.steps[0].label, "Open tunnel to 10.0.0.5:5432");
    let mut direct = Steps::default();
    direct.database(&run(&trace, "10.0.0.5", false), None);
    assert_eq!(direct.steps[1].status, StepStatus::Warning);
}

#[test]
fn a_failed_handshake_is_a_typed_tls_step() {
    let trace = ConnectTrace {
        completed: completed(&[ConnectStage::Connect]),
        failed: Some(ConnectStage::Tls),
        ..ConnectTrace::default()
    };
    let error = DriverError::Tls("the server certificate is not signed by a trusted CA".to_owned());
    let mut steps = Steps::default();
    steps.database(&run(&trace, "db.example.com", false), Some(&error));
    assert_eq!(
        steps.steps.last().map(|step| (step.kind, step.status)),
        Some((TestStepKind::Tls, StepStatus::Failed))
    );
    assert_eq!(IpcError::from(&error).kind, IpcErrorKind::Tls);
}

#[test]
fn a_server_without_tls_under_prefer_warns() {
    let trace = ConnectTrace {
        completed: completed(&[ConnectStage::Connect, ConnectStage::Tls]),
        tls: Some(TlsStatus::PlaintextFallback),
        ..ConnectTrace::default()
    };
    let mut steps = Steps::default();
    steps.database(&run(&trace, "localhost", false), None);
    assert_eq!(steps.steps[1].status, StepStatus::Warning);
}

#[test]
fn a_direct_connect_reports_dns_tcp_tls_auth_and_version_in_order() {
    let trace = ConnectTrace {
        completed: completed(&[
            ConnectStage::Resolve,
            ConnectStage::Connect,
            ConnectStage::Tls,
            ConnectStage::Authenticate,
            ConnectStage::ServerInfo,
        ]),
        tls: Some(TlsStatus::Encrypted(details(Verification::Full))),
        ..ConnectTrace::default()
    };
    let server = ServerDto {
        product: "PostgreSQL".to_owned(),
        server_version: "17.2".to_owned(),
        flavour: None,
        database: "app".to_owned(),
    };
    let mut steps = Steps::default();
    let run = DatabaseRun {
        server: Some(&server),
        ..run(&trace, "db.example.com", false)
    };
    steps.database(&run, None);
    let kinds: Vec<TestStepKind> = steps.steps.iter().map(|step| step.kind).collect();
    assert_eq!(
        kinds,
        [
            TestStepKind::Dns,
            TestStepKind::Connect,
            TestStepKind::Tls,
            TestStepKind::Authenticate,
            TestStepKind::Version,
        ]
    );
    assert!(steps.steps.iter().all(|step| step.elapsed_ms == Some(3)));
    assert_eq!(steps.steps[4].detail.as_deref(), Some("PostgreSQL 17.2"));
}

#[test]
fn an_unknown_host_fails_the_dns_step_with_a_hint() {
    let trace = ConnectTrace {
        failed: Some(ConnectStage::Resolve),
        ..ConnectTrace::default()
    };
    let error = DriverError::Network(io::Error::new(
        io::ErrorKind::NotFound,
        "resolving db.nowhere: failed to lookup address",
    ));
    let mut steps = Steps::default();
    steps.database(&run(&trace, "db.nowhere", false), Some(&error));
    let step = &steps.steps[0];
    assert_eq!(
        (step.kind, step.status, step.label.as_str()),
        (TestStepKind::Dns, StepStatus::Failed, "Resolve db.nowhere")
    );
    assert!(
        step.hint
            .as_deref()
            .is_some_and(|hint| hint.contains("host name"))
    );
}
