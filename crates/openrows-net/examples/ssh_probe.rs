// SPDX-License-Identifier: Apache-2.0
//! Manual probe for a real bastion (M0-S4 maintainer run).
//!
//! ```text
//! cargo run -p openrows-net --example ssh_probe -- <alias> [<target-host> <target-port>]
//! ```
//!
//! Resolves `<alias>` from `~/.ssh/config` (including `ProxyJump`), verifies
//! host keys against `~/.ssh/known_hosts` (read-only) plus the file named by
//! `OPENROWS_KNOWN_HOSTS` (default: a file in the temp directory), asks on the
//! terminal for first-use and keyboard-interactive answers, prints per-hop
//! timings, and optionally opens a tunnel to `<target-host>:<target-port>`.
//! Keyboard-interactive answers are echoed; use it for OTP codes, not passwords.
//! `OPENROWS_SSH_HOME` replaces the home directory (to try it on fixtures).
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "interactive command-line example"
)]

use std::{
    env, io,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::Instant,
};

use openrows_net::{
    BoxFuture, Challenge, FirstUseDecision, HostKeyPolicy, KeyboardInteractivePrompter,
    PromptCancelled, SecretString, SshHop, SshTunnel, Timeouts, TunnelOptions, UnknownHost,
    config::SshConfig, known_hosts::KnownHosts, resolve_route,
};

struct Terminal;

/// Read one line from stdin off the async runtime.
async fn ask(question: String) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        eprint!("{question}");
        let mut line = String::new();
        io::stdin()
            .read_line(&mut line)
            .ok()
            .filter(|read| *read > 0)?;
        Some(line.trim_end_matches(['\r', '\n']).to_owned())
    })
    .await
    .ok()
    .flatten()
}

impl HostKeyPolicy for Terminal {
    fn on_unknown_host<'a>(&'a self, host: &'a UnknownHost) -> BoxFuture<'a, FirstUseDecision> {
        Box::pin(async move {
            if !host.other_algorithms.is_empty() {
                eprintln!(
                    "WARNING: other key types are already known for this host: {:?}",
                    host.other_algorithms
                );
            }
            let question = format!(
                "Unknown host {}:{} ({} {}). Trust? [y = remember / o = once / N] ",
                host.host, host.port, host.algorithm, host.fingerprint
            );
            match ask(question).await.as_deref() {
                Some("y" | "Y") => FirstUseDecision::AcceptAndRemember,
                Some("o" | "O") => FirstUseDecision::AcceptOnce,
                _ => FirstUseDecision::Reject,
            }
        })
    }
}

impl KeyboardInteractivePrompter for Terminal {
    fn respond<'a>(
        &'a self,
        challenge: &'a Challenge,
    ) -> BoxFuture<'a, Result<Vec<SecretString>, PromptCancelled>> {
        Box::pin(async move {
            for text in [&challenge.name, &challenge.instructions]
                .into_iter()
                .filter(|text| !text.is_empty())
            {
                eprintln!("{text}");
            }
            let mut answers = Vec::with_capacity(challenge.prompts.len());
            for prompt in &challenge.prompts {
                let label = format!("[{}@{}] {}", challenge.user, challenge.host, prompt.text);
                answers.push(SecretString::new(ask(label).await.ok_or(PromptCancelled)?));
            }
            Ok(answers)
        })
    }
}

/// Hops for `alias` from `<home>/.ssh/config`.
fn route(home: &Path, alias: &str) -> Result<Vec<SshHop>, String> {
    let config =
        SshConfig::load(&home.join(".ssh/config"), home).map_err(|error| error.to_string())?;
    let local_user = env::var("USER").unwrap_or_default();
    Ok(resolve_route(&config, alias)
        .map_err(|error| error.to_string())?
        .iter()
        .map(|settings| SshHop::from_settings(settings, &local_user))
        .collect())
}

/// Read the user's `known_hosts`; record accepted keys in the probe's own file.
fn options(home: &Path) -> TunnelOptions {
    let learn_to = env::var_os("OPENROWS_KNOWN_HOSTS").map_or_else(
        || env::temp_dir().join("openrows-probe-known_hosts"),
        PathBuf::from,
    );
    println!("app known_hosts: {}", learn_to.display());
    TunnelOptions {
        known_hosts: Arc::new(KnownHosts::new(
            vec![home.join(".ssh/known_hosts")],
            learn_to,
        )),
        host_key_policy: Arc::new(Terminal),
        prompter: Arc::new(Terminal),
        timeouts: Timeouts::default(),
    }
}

async fn run(alias: &str, target: Option<(&str, u16)>) -> Result<(), String> {
    let home = env::var_os("OPENROWS_SSH_HOME")
        .map(PathBuf::from)
        .or_else(env::home_dir)
        .ok_or("no home directory")?;
    let hops = route(&home, alias)?;
    let labels: Vec<String> = hops
        .iter()
        .map(|hop| format!("{}@{}:{}", hop.user, hop.host, hop.port))
        .collect();
    println!("route: {}", labels.join(" -> "));
    let tunnel = SshTunnel::connect(&hops, &options(&home))
        .await
        .map_err(|error| error.to_string())?;
    for hop in tunnel.timings() {
        println!(
            "hop {}: connect {:?}, handshake+auth {:?} (includes prompts)",
            hop.host, hop.connect, hop.handshake
        );
    }
    if let Some((host, port)) = target {
        let started = Instant::now();
        let stream = tunnel
            .open_stream(host, port)
            .await
            .map_err(|error| error.to_string())?;
        drop(stream);
        println!("tunnel to {host}:{port} opened in {:?}", started.elapsed());
    }
    tunnel.close().await.map_err(|error| error.to_string())
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let target = match args.get(1..3) {
        Some([host, port]) => port.parse().ok().map(|port| (host.as_str(), port)),
        _ => None,
    };
    let Some(alias) = args.first().filter(|_| args.len() == 1 || target.is_some()) else {
        eprintln!("usage: ssh_probe <alias> [<target-host> <target-port>]");
        return ExitCode::from(2);
    };
    match run(alias, target).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
