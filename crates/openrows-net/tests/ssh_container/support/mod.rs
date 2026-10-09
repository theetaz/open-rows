// SPDX-License-Identifier: Apache-2.0
//! Fixtures for the OpenSSH container tests: runtime-generated keys and
//! passwords, a throwaway ssh-agent, scripted UI callbacks and an echo probe.
#![allow(
    clippy::expect_used,
    reason = "test fixtures fail fast with a message, like the tests that call them"
)]

mod docker;

use std::{
    collections::hash_map::RandomState,
    fs,
    hash::BuildHasher,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub use docker::{Container, ContainerSpec, Network, docker_ready};
use openrows_net::{
    BoxFuture, Challenge, FirstUseDecision, HostKeyPolicy, KeyboardInteractivePrompter,
    PromptCancelled, SecretString, SshHop, SshTunnel, Timeouts, TunnelOptions, UnknownHost,
    config::SshConfig, known_hosts::KnownHosts,
};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Informational output for the acceptance report; visible with `--no-capture`.
#[allow(
    clippy::print_stderr,
    reason = "test diagnostics: skip notices and handshake timings"
)]
pub fn note(message: &str) {
    eprintln!("[openrows-net ssh] {message}");
}

/// A synthetic, per-process random password (never a real credential).
pub fn password() -> String {
    format!(
        "pw-{:016x}",
        RandomState::new().hash_one(std::process::id())
    )
}

/// A generated key pair.
pub struct GeneratedKey {
    pub private: PathBuf,
    pub public: String,
}

/// Temp directory for keys, `known_hosts`, config and the agent socket.
pub struct Workspace {
    dir: TempDir,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("temp dir"),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn known_hosts(&self) -> PathBuf {
        self.path().join("known_hosts")
    }

    /// `ssh-keygen -t … -N <passphrase>` into the workspace.
    pub fn keygen(&self, name: &str, args: &[&str], passphrase: &str) -> GeneratedKey {
        let private = self.path().join(name);
        let status = Command::new("ssh-keygen")
            .args(["-q", "-C", name, "-N", passphrase, "-f"])
            .arg(&private)
            .args(args)
            .status()
            .expect("ssh-keygen runs");
        assert!(status.success(), "ssh-keygen {args:?}");
        let public = fs::read_to_string(format!("{}.pub", private.display())).expect("public key");
        GeneratedKey {
            private,
            public: public.trim().to_owned(),
        }
    }

    /// Write `text` as an SSH config file in the workspace and load it.
    pub fn ssh_config(&self, text: &str) -> SshConfig {
        let path = self.path().join("ssh_config");
        fs::write(&path, text).expect("write config");
        SshConfig::load(&path, self.path()).expect("config parses")
    }

    /// Hash every host name in place, like `ssh-keygen -H`.
    pub fn hash_known_hosts(&self) {
        let status = Command::new("ssh-keygen")
            .arg("-q")
            .arg("-H")
            .arg("-f")
            .arg(self.known_hosts())
            .stdout(Stdio::null())
            .status()
            .expect("ssh-keygen runs");
        assert!(status.success(), "ssh-keygen -H");
    }

    pub fn options(&self, policy: Arc<Policy>, prompter: Arc<Prompter>) -> TunnelOptions {
        TunnelOptions {
            known_hosts: Arc::new(KnownHosts::new(Vec::new(), self.known_hosts())),
            host_key_policy: policy,
            prompter,
            timeouts: Timeouts::default(),
            proxy: None,
        }
    }
}

/// First-use policy that records every request and answers `decision`.
pub struct Policy {
    decision: FirstUseDecision,
    seen: Mutex<Vec<UnknownHost>>,
}

impl Policy {
    pub fn new(decision: FirstUseDecision) -> Arc<Self> {
        Arc::new(Self {
            decision,
            seen: Mutex::new(Vec::new()),
        })
    }

    pub fn seen(&self) -> Vec<UnknownHost> {
        self.seen.lock().expect("lock").clone()
    }
}

impl HostKeyPolicy for Policy {
    fn on_unknown_host<'a>(&'a self, host: &'a UnknownHost) -> BoxFuture<'a, FirstUseDecision> {
        self.seen.lock().expect("lock").push(host.clone());
        Box::pin(async move { self.decision })
    }
}

/// Answers every prompt with `answer` (or cancels when `None`), recording challenges.
pub struct Prompter {
    answer: Option<SecretString>,
    seen: Mutex<Vec<Challenge>>,
}

impl Prompter {
    pub fn answering(answer: Option<&str>) -> Arc<Self> {
        Arc::new(Self {
            answer: answer.map(SecretString::from),
            seen: Mutex::new(Vec::new()),
        })
    }

    pub fn seen(&self) -> Vec<Challenge> {
        self.seen.lock().expect("lock").clone()
    }
}

impl KeyboardInteractivePrompter for Prompter {
    fn respond<'a>(
        &'a self,
        challenge: &'a Challenge,
    ) -> BoxFuture<'a, Result<Vec<SecretString>, PromptCancelled>> {
        self.seen.lock().expect("lock").push(challenge.clone());
        let answers = self
            .answer
            .as_ref()
            .map(|answer| vec![answer.clone(); challenge.prompts.len()]);
        Box::pin(async move { answers.ok_or(PromptCancelled) })
    }
}

/// A throwaway `ssh-agent -D` holding `keys`; killed on drop.
pub struct Agent {
    child: Child,
    pub socket: PathBuf,
}

impl Agent {
    pub fn start(dir: &Path, keys: &[&Path]) -> Self {
        let socket = dir.join("agent.sock");
        let child = Command::new("ssh-agent")
            .arg("-D")
            .arg("-a")
            .arg(&socket)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("ssh-agent starts");
        let agent = Self { child, socket };
        let deadline = Instant::now() + Duration::from_secs(5);
        while !agent.socket.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        for key in keys {
            let status = Command::new("ssh-add")
                .arg("-q")
                .arg(key)
                .env("SSH_AUTH_SOCK", &agent.socket)
                .stderr(Stdio::null())
                .status()
                .expect("ssh-add runs");
            assert!(status.success(), "ssh-add {}", key.display());
        }
        agent
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Informational tunnel timings for one probe.
pub struct EchoTiming {
    /// First channel open after authentication.
    pub first_open: Duration,
    /// A second channel open on the warm session.
    pub warm_open: Duration,
    /// One echo round trip through the forwarded stream.
    pub round_trip: Duration,
}

/// Open `127.0.0.1:7000` through the tunnel, check the greeting names
/// `expected_host`, echo a payload, and measure open and round-trip times.
pub async fn echo_through(tunnel: &SshTunnel, expected_host: &str) -> EchoTiming {
    let started = Instant::now();
    let mut stream = tunnel
        .open_stream("127.0.0.1", 7000)
        .await
        .expect("open forwarded stream");
    let first_open = started.elapsed();
    let mut greeting = vec![0; expected_host.len() + 1];
    stream.read_exact(&mut greeting).await.expect("greeting");
    assert_eq!(
        String::from_utf8_lossy(&greeting),
        format!("{expected_host}\n")
    );
    let payload = b"openrows-echo-payload-0123456789";
    let mut echoed = vec![0; payload.len()];
    let started = Instant::now();
    stream.write_all(payload).await.expect("write");
    stream.read_exact(&mut echoed).await.expect("echo");
    let round_trip = started.elapsed();
    assert_eq!(&echoed, payload);
    let started = Instant::now();
    drop(
        tunnel
            .open_stream("127.0.0.1", 7000)
            .await
            .expect("second stream"),
    );
    EchoTiming {
        first_open,
        warm_open: started.elapsed(),
        round_trip,
    }
}

/// Connect, echo through the last hop (greeting `expected_host`), report, close.
pub async fn login_and_echo(
    hops: &[SshHop],
    options: &TunnelOptions,
    expected_host: &str,
    label: &str,
) {
    let tunnel = SshTunnel::connect(hops, options)
        .await
        .map_err(|error| format!("{label}: {error}"))
        .expect("login");
    report(label, &tunnel, &echo_through(&tunnel, expected_host).await);
    tunnel.close().await.expect("close");
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1e3
}

/// Print per-hop and channel timings for the acceptance report.
pub fn report(label: &str, tunnel: &SshTunnel, echo: &EchoTiming) {
    for hop in tunnel.timings() {
        note(&format!(
            "timing {label}: hop={} connect={:.1}ms handshake+auth={:.1}ms",
            hop.host,
            millis(hop.connect),
            millis(hop.handshake)
        ));
    }
    note(&format!(
        "timing {label}: first-channel-open={:.1}ms warm-channel-open={:.2}ms echo-rtt={:.2}ms",
        millis(echo.first_open),
        millis(echo.warm_open),
        millis(echo.round_trip)
    ));
}
