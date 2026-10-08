// SPDX-License-Identifier: Apache-2.0
use std::path::PathBuf;

use openrows_net::known_hosts::KnownHostsLine;

use super::*;

fn settings(auth: SshAuthMethod, jumps: &[&str]) -> SshSettings {
    SshSettings {
        host: "bastion".to_owned(),
        port: None,
        user: None,
        auth,
        jump_hosts: jumps.iter().map(|jump| (*jump).to_owned()).collect(),
    }
}

#[test]
fn profile_settings_become_a_route_spec_with_keychain_secrets() -> Result<(), IpcError> {
    let secrets = SshSecrets {
        password: None,
        passphrase: Some(StoredSecret::from("synthetic")),
    };
    let key = SshAuthMethod::PrivateKey {
        key_file: PathBuf::from("~/.ssh/id_ed25519"),
    };
    let spec = route_spec(&settings(key, &["ops@jump:2222", "edge"]), secrets)?;
    assert_eq!(spec.jump_hosts.len(), 2);
    assert_eq!(spec.jump_hosts[0].port, Some(2222));
    assert!(matches!(
        spec.auth,
        SshAuth::PrivateKey { passphrase: Some(ref secret), .. } if secret.expose() == "synthetic"
    ));
    let bad = route_spec(
        &settings(SshAuthMethod::Agent, &["@nohost"]),
        SshSecrets::default(),
    );
    assert!(bad.is_err_and(|error| error.kind == IpcErrorKind::Invalid
        && error.message.starts_with("Jump host @nohost")));
    Ok(())
}

#[test]
fn a_changed_host_key_is_a_hard_stop_with_an_explanation() {
    let error = ssh_error(&SshError::HostKeyChanged {
        host: "bastion.example.com".to_owned(),
        fingerprint: "SHA256:new".to_owned(),
        at: KnownHostsLine {
            path: PathBuf::from("/home/me/.ssh/known_hosts"),
            line: 3,
        },
    });
    assert_eq!(error.kind, IpcErrorKind::HostKey);
    assert!(error.message.contains("has changed"));
    assert!(error.message.contains("will not connect"));
    assert!(error.message.contains("/home/me/.ssh/known_hosts:3"));
}

#[test]
fn dismissed_prompts_are_cancellations_and_other_failures_are_ssh_errors() {
    let rejected = ssh_error(&SshError::HostKeyRejected {
        host: "bastion".to_owned(),
        fingerprint: "SHA256:x".to_owned(),
    });
    assert_eq!(rejected.kind, IpcErrorKind::Cancelled);
    assert_eq!(
        ssh_error(&SshError::PromptCancelled).kind,
        IpcErrorKind::Cancelled
    );
    let auth = ssh_error(&SshError::AuthFailed {
        user: "ops".to_owned(),
        host: "bastion".to_owned(),
        tried: "password".to_owned(),
    });
    assert_eq!(auth.kind, IpcErrorKind::Ssh);
    assert!(auth.message.contains("ops@bastion"));
}

#[test]
fn failed_opens_are_not_shared() {
    let tunnels = Tunnels::default();
    let failure = tauri::async_runtime::block_on(tunnels.get_or_open("p", async {
        Err(IpcError::new(IpcErrorKind::Ssh, "SSH: refused"))
    }));
    assert!(failure.is_err());
    let again = tauri::async_runtime::block_on(tunnels.get_or_open("p", async {
        Err(IpcError::new(IpcErrorKind::Ssh, "SSH: still refused"))
    }));
    assert!(again.is_err_and(|error| error.message == "SSH: still refused"));
    tunnels.forget("p");
}
