// SPDX-License-Identifier: Apache-2.0
//! Authentication methods: password, key files, agent, keyboard-interactive.

use openrows_net::{AgentSource, AuthMethod, FirstUseDecision, SshError, SshTunnel};

use crate::support::{
    Agent, Container, GeneratedKey, Policy, Prompter, Workspace, docker_ready, login_and_echo,
    password,
};
use crate::{hop, published};

const PASSPHRASE: &str = "synthetic-key-passphrase";

/// Key formats to generate: name, `ssh-keygen` arguments, passphrase.
const KEY_CASES: [(&str, &[&str], &str); 8] = [
    ("ed25519", &["-t", "ed25519"], ""),
    ("ecdsa", &["-t", "ecdsa"], ""),
    ("ecdsa-pem", &["-t", "ecdsa", "-m", "PEM"], ""),
    ("rsa", &["-t", "rsa", "-b", "3072"], ""),
    ("rsa-pem", &["-t", "rsa", "-b", "2048", "-m", "PEM"], ""),
    ("rsa-pkcs8", &["-t", "rsa", "-b", "2048", "-m", "PKCS8"], ""),
    ("ed25519-encrypted", &["-t", "ed25519"], PASSPHRASE),
    ("rsa-encrypted", &["-t", "rsa", "-b", "3072"], PASSPHRASE),
];

fn key_auth(key: &GeneratedKey, passphrase: &str) -> Vec<AuthMethod> {
    vec![AuthMethod::PrivateKey {
        path: key.private.clone(),
        passphrase: (!passphrase.is_empty()).then(|| passphrase.into()),
    }]
}

#[tokio::test]
async fn password_auth_first_use_then_hashed_known_hosts() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    let target = hop(
        &server,
        "tester",
        vec![AuthMethod::Password(secret.as_str().into())],
    );

    let accept = Policy::new(FirstUseDecision::AcceptAndRemember);
    let options = work.options(accept.clone(), Prompter::answering(None));
    login_and_echo(
        std::slice::from_ref(&target),
        &options,
        &server.name,
        "password",
    )
    .await;
    let seen = accept.seen();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].fingerprint.starts_with("SHA256:"));
    assert_eq!(seen[0].algorithm, "ssh-ed25519");

    work.hash_known_hosts();
    let stored = std::fs::read_to_string(work.known_hosts()).expect("known_hosts");
    assert!(stored.starts_with("|1|"), "entry is hashed: {stored}");
    let strict = Policy::new(FirstUseDecision::Reject);
    let options = work.options(strict.clone(), Prompter::answering(None));
    let tunnel = SshTunnel::connect(&[target], &options)
        .await
        .expect("hashed entry is trusted");
    tunnel.close().await.expect("close");
    assert!(strict.seen().is_empty(), "a trusted host must not prompt");
}

#[tokio::test]
async fn private_key_matrix_including_encrypted_keys() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let keys: Vec<_> = KEY_CASES
        .iter()
        .map(|(name, args, phrase)| (*name, work.keygen(name, args, phrase), *phrase))
        .collect();
    let authorized: Vec<&str> = keys.iter().map(|(_, key, _)| key.public.as_str()).collect();
    let server = Container::start(&published(&password(), &authorized.join("\n")));
    let options = work.options(
        Policy::new(FirstUseDecision::AcceptOnce),
        Prompter::answering(None),
    );

    for (name, key, phrase) in &keys {
        let hops = [hop(&server, "tester", key_auth(key, phrase))];
        login_and_echo(&hops, &options, &server.name, name).await;
    }

    let (_, encrypted, _) = keys
        .iter()
        .find(|(_, _, phrase)| !phrase.is_empty())
        .expect("encrypted key");
    let locked = hop(&server, "tester", key_auth(encrypted, ""));
    let result = SshTunnel::connect(&[locked], &options).await;
    assert!(
        matches!(result, Err(SshError::PassphraseRequired { .. })),
        "{result:?}"
    );
}

#[tokio::test]
async fn ssh_agent_offers_identities_until_one_is_accepted() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let unrelated = work.keygen("agent-ed25519", &["-t", "ed25519"], "");
    let accepted = work.keygen("agent-rsa", &["-t", "rsa", "-b", "3072"], "");
    let agent = Agent::start(work.path(), &[&unrelated.private, &accepted.private]);
    let server = Container::start(&published(&password(), &accepted.public));

    let auth = vec![AuthMethod::Agent(AgentSource::Socket(agent.socket.clone()))];
    let options = work.options(
        Policy::new(FirstUseDecision::AcceptOnce),
        Prompter::answering(None),
    );
    login_and_echo(
        &[hop(&server, "tester", auth)],
        &options,
        &server.name,
        "agent-rsa",
    )
    .await;
}

#[tokio::test]
async fn keyboard_interactive_prompts_go_through_the_callback() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let server = Container::start(&published(&secret, ""));
    let prompter = Prompter::answering(Some(&secret));
    let options = work.options(Policy::new(FirstUseDecision::AcceptOnce), prompter.clone());

    let hops = [hop(
        &server,
        "tester",
        vec![AuthMethod::KeyboardInteractive],
    )];
    login_and_echo(&hops, &options, &server.name, "keyboard-interactive").await;
    let challenge = prompter.seen().into_iter().next().expect("prompted");
    assert!(
        challenge.prompts[0].text.contains("assword"),
        "{challenge:?}"
    );
    assert!(!challenge.prompts[0].echo);
}

/// The fixture's "mfa" user needs publickey followed by keyboard-interactive.
#[tokio::test]
async fn mfa_requires_key_then_one_time_code() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let secret = password();
    let key = work.keygen("mfa-key", &["-t", "ed25519"], "");
    let server = Container::start(&published(&secret, &key.public));
    let options = work.options(
        Policy::new(FirstUseDecision::AcceptOnce),
        Prompter::answering(Some(&secret)),
    );

    let mut both = key_auth(&key, "");
    both.push(AuthMethod::KeyboardInteractive);
    login_and_echo(
        &[hop(&server, "mfa", both)],
        &options,
        &server.name,
        "key+otp",
    )
    .await;
    let only_code = hop(&server, "mfa", vec![AuthMethod::KeyboardInteractive]);
    let result = SshTunnel::connect(&[only_code], &options).await;
    assert!(
        matches!(result, Err(SshError::AuthFailed { .. })),
        "{result:?}"
    );
}

#[tokio::test]
async fn wrong_credentials_and_cancelled_prompts_are_typed() {
    if !docker_ready() {
        return;
    }
    let work = Workspace::new();
    let server = Container::start(&published(&password(), ""));
    let options = work.options(
        Policy::new(FirstUseDecision::AcceptOnce),
        Prompter::answering(None),
    );
    let wrong = hop(
        &server,
        "tester",
        vec![AuthMethod::Password("not-the-password".into())],
    );
    let result = SshTunnel::connect(&[wrong], &options).await;
    assert!(
        matches!(&result, Err(SshError::AuthFailed { tried, .. }) if tried == "password"),
        "{result:?}"
    );
    let interactive = hop(&server, "tester", vec![AuthMethod::KeyboardInteractive]);
    let cancelled = SshTunnel::connect(&[interactive], &options).await;
    assert!(
        matches!(cancelled, Err(SshError::PromptCancelled)),
        "{cancelled:?}"
    );
}
