// SPDX-License-Identifier: Apache-2.0
//! Throwaway OpenSSH containers and networks, removed on drop.

use std::{
    process::{Command, Output},
    sync::{
        OnceLock,
        atomic::{AtomicU32, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// Local tag for the fixture image built from `tests/fixtures/sshd`.
const IMAGE: &str = "openrows-test-sshd:m1-06";
/// When set, missing Docker fails the run instead of skipping (`just test-ssh`).
const REQUIRE_ENV: &str = "OPENROWS_REQUIRE_DOCKER";
const READY_TIMEOUT: Duration = Duration::from_secs(30);

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("docker CLI runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Whether container tests can run; builds the image once per process.
///
/// Returns `false` (test skips) when Docker is unavailable, unless
/// `OPENROWS_REQUIRE_DOCKER` is set, in which case it fails loudly.
pub fn docker_ready() -> bool {
    static READY: OnceLock<bool> = OnceLock::new();
    *READY.get_or_init(|| {
        let available = Command::new("docker")
            .args(["info", "--format", "{{.ServerVersion}}"])
            .output()
            .is_ok_and(|output| output.status.success());
        if !available {
            assert!(
                std::env::var_os(REQUIRE_ENV).is_none(),
                "{REQUIRE_ENV} is set but Docker is unavailable"
            );
            super::note("skipped: Docker is unavailable");
            return false;
        }
        let context = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/sshd");
        let build = docker(&["build", "-q", "-t", IMAGE, context]);
        assert!(
            build.status.success(),
            "image build failed: {}",
            String::from_utf8_lossy(&build.stderr)
        );
        true
    })
}

fn unique_name(kind: &str) -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    format!(
        "openrows-s4-{kind}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// A private Docker network for multi-hop tests.
pub struct Network {
    pub name: String,
}

impl Network {
    pub fn create() -> Self {
        let name = unique_name("net");
        let output = docker(&["network", "create", &name]);
        assert!(output.status.success(), "network create failed");
        Self { name }
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        let _ = docker(&["network", "rm", &self.name]);
    }
}

/// How to start one sshd container.
pub struct ContainerSpec<'a> {
    pub password: &'a str,
    pub authorized_keys: &'a str,
    /// Publish port 22 on an ephemeral 127.0.0.1 port.
    pub publish: bool,
    /// Attach to a network under an alias.
    pub network: Option<(&'a Network, &'a str)>,
}

/// A running sshd; its hostname equals `name`.
pub struct Container {
    pub name: String,
}

impl Container {
    pub fn start(spec: &ContainerSpec<'_>) -> Self {
        let name = unique_name("sshd");
        let password = format!("TEST_PASSWORD={}", spec.password);
        let keys = format!("AUTHORIZED_KEYS={}", spec.authorized_keys);
        let mut args = vec![
            "run",
            "-d",
            "--rm",
            "--name",
            &name,
            "--hostname",
            &name,
            "-e",
            &password,
            "-e",
            &keys,
        ];
        if spec.publish {
            args.extend(["-p", "127.0.0.1::22"]);
        }
        if let Some((network, alias)) = spec.network {
            args.extend(["--network", &network.name, "--network-alias", alias]);
        }
        args.push(IMAGE);
        let output = docker(&args);
        assert!(
            output.status.success(),
            "docker run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let container = Self { name };
        container.wait_ready();
        container
    }

    /// Host port mapped to the container's port 22.
    pub fn port(&self) -> u16 {
        let output = docker(&["port", &self.name, "22"]);
        let mapping = stdout(&output);
        let first = mapping.lines().next().unwrap_or_default();
        first
            .rsplit(':')
            .next()
            .and_then(|port| port.parse().ok())
            .expect("published port")
    }

    /// Run `command` inside the container (e.g. to cut SSH sessions).
    pub fn exec(&self, command: &[&str]) -> bool {
        let mut args = vec!["exec", self.name.as_str()];
        args.extend(command);
        docker(&args).status.success()
    }

    /// Stop the container abruptly; the name stays reserved until drop.
    pub fn kill(&self) {
        let output = docker(&["kill", &self.name]);
        assert!(output.status.success(), "docker kill failed");
    }

    /// Poll the log for sshd's listening line (bounded; not a fixed sleep).
    fn wait_ready(&self) {
        let deadline = Instant::now() + READY_TIMEOUT;
        let mut ready = false;
        while !ready && Instant::now() < deadline {
            let logs = docker(&["logs", &self.name]);
            ready = String::from_utf8_lossy(&logs.stderr).contains("Server listening");
            if !ready {
                thread::sleep(Duration::from_millis(100));
            }
        }
        assert!(ready, "sshd in {} did not start", self.name);
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        let _ = docker(&["rm", "-f", &self.name]);
    }
}
