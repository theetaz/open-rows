// SPDX-License-Identifier: Apache-2.0
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

use proptest::prelude::*;

use super::{ConfigError, ConfigSource, JumpSpec, SshConfig};

const HOME: &str = "/home/fixture";

/// In-memory file tree; nothing touches the real home directory.
#[derive(Default)]
struct Files(BTreeMap<PathBuf, String>);

impl Files {
    fn with(mut self, path: &str, text: &str) -> Self {
        self.0.insert(PathBuf::from(path), text.to_owned());
        self
    }
}

impl ConfigSource for Files {
    fn read(&self, path: &Path) -> io::Result<String> {
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn list(&self, dir: &Path) -> io::Result<Vec<PathBuf>> {
        Ok(self
            .0
            .keys()
            .filter(|path| path.parent() == Some(dir))
            .cloned()
            .collect())
    }
}

fn parse_with(text: &str, files: &Files) -> Result<SshConfig, ConfigError> {
    SshConfig::parse(
        text,
        Path::new("/home/fixture/.ssh/config"),
        Path::new(HOME),
        files,
    )
}

fn parse(text: &str) -> SshConfig {
    parse_with(text, &Files::default()).expect("fixture config parses")
}

const SAMPLE: &str = r#"
# Production database bastion
Host prod-db
    HostName db.internal.example
    User app
    Port 2222
    IdentityFile ~/.ssh/id_prod
    IdentitiesOnly yes
    ProxyJump ops@bastion.example:2022,edge

Host *.example !secret.example
    User=wildcard
    IdentityFile "~/.ssh/id with space"

Host *
    User fallback
    Port 22
    ServerAliveInterval 30
"#;

#[test]
fn resolves_alias_with_first_value_winning() {
    let settings = parse(SAMPLE).resolve("prod-db");
    assert_eq!(settings.host_name, "db.internal.example");
    assert_eq!(settings.user.as_deref(), Some("app"));
    assert_eq!(settings.port, 2222);
    assert_eq!(
        settings.identity_files,
        vec![PathBuf::from("/home/fixture/.ssh/id_prod")]
    );
    assert!(settings.identities_only);
    assert_eq!(
        settings.proxy_jump,
        vec![
            JumpSpec {
                user: Some("ops".into()),
                host: "bastion.example".into(),
                port: Some(2022)
            },
            JumpSpec {
                user: None,
                host: "edge".into(),
                port: None
            },
        ]
    );
}

#[test]
fn wildcards_negation_and_quoted_identity_files() {
    let config = parse(SAMPLE);
    let matched = config.resolve("api.example");
    assert_eq!(matched.user.as_deref(), Some("wildcard"));
    assert_eq!(
        matched.identity_files,
        vec![PathBuf::from("/home/fixture/.ssh/id with space")]
    );
    let negated = config.resolve("secret.example");
    assert_eq!(negated.user.as_deref(), Some("fallback"));
    assert!(negated.identity_files.is_empty());
}

#[test]
fn unknown_alias_gets_defaults() {
    let settings = parse("").resolve("plain-host");
    assert_eq!(settings.host_name, "plain-host");
    assert_eq!(settings.port, 22);
    assert_eq!(settings.user, None);
    assert!(settings.proxy_jump.is_empty());
}

#[test]
fn host_name_tokens_and_proxy_jump_none() {
    let config = parse(
        "Host *.lan\n  HostName %h.corp.example\n  ProxyJump none\nHost *\n  ProxyJump bastion\n",
    );
    let settings = config.resolve("db.lan");
    assert_eq!(settings.host_name, "db.lan.corp.example");
    assert!(settings.proxy_jump.is_empty(), "first ProxyJump none wins");
    assert_eq!(config.resolve("other").proxy_jump.len(), 1);
}

#[test]
fn match_blocks_never_apply() {
    let settings =
        parse("Match host x\n  User from-match\nHost *\n  User from-host\n").resolve("x");
    assert_eq!(settings.user.as_deref(), Some("from-host"));
}

#[test]
fn include_is_relative_to_ssh_dir_and_globbed() {
    let files = Files::default()
        .with(
            "/home/fixture/.ssh/config.d/10-db",
            "Host db\n  User included\n",
        )
        .with(
            "/home/fixture/.ssh/config.d/20-other",
            "Host other\n  Port 2200\n",
        )
        .with(
            "/home/fixture/.ssh/config.d/ignored.txt",
            "Host db\n  User wrong\n",
        );
    let config =
        parse_with("Include config.d/??-*\nHost db\n  User later\n", &files).expect("parses");
    assert_eq!(config.resolve("db").user.as_deref(), Some("included"));
    assert_eq!(config.resolve("other").port, 2200);
}

#[test]
fn include_inside_inactive_host_is_skipped_and_state_restored() {
    let files = Files::default().with("/inc", "User from-include\nHost nomatch-inner\n");
    let text = "Host nomatch\n  Include /inc\nHost db\n  Include /inc\n  Port 2201\n";
    let config = parse_with(text, &files).expect("parses");
    let db = config.resolve("db");
    assert_eq!(db.user.as_deref(), Some("from-include"));
    assert_eq!(
        db.port, 2201,
        "a Host line inside the include must not leak out"
    );
    assert_eq!(config.resolve("other").user, None);
}

#[test]
fn include_cycles_are_rejected() {
    let files = Files::default().with("/loop", "Include /loop\n");
    let result = parse_with("Include /loop\n", &files);
    assert!(matches!(
        result,
        Err(ConfigError::IncludeDepth { max: 16, .. })
    ));
}

#[test]
fn syntax_errors_report_file_and_line() {
    for text in [
        "Host a\n  Port nope\n",
        "Host a\n  IdentitiesOnly maybe\n",
        "Host a\n  User \"open\n",
        "Host\n",
    ] {
        let error = parse_with(text, &Files::default()).expect_err("must fail");
        assert!(
            matches!(error, ConfigError::Syntax { line, .. } if line >= 1),
            "{text:?}"
        );
    }
}

#[test]
fn missing_file_loads_as_empty_config() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = SshConfig::load(&dir.path().join("absent"), dir.path()).expect("empty");
    assert_eq!(config.resolve("x").host_name, "x");
}

proptest! {
    #[test]
    fn parser_never_panics(text in "(?s).{0,400}", alias in "[a-z.*!-]{0,16}") {
        if let Ok(config) = parse_with(&text, &Files::default()) {
            let _ = config.resolve(&alias);
        }
    }

    #[test]
    fn generated_host_blocks_round_trip(
        alias in "[a-z][a-z0-9-]{0,12}",
        host in "[a-z][a-z0-9.-]{0,20}",
        user in "[a-z_][a-z0-9_]{0,8}",
        port in 1u16..,
    ) {
        let text = format!("Host {alias}\n  HostName {host}\n  User {user}\n  Port {port}\n");
        let settings = parse(&text).resolve(&alias);
        prop_assert_eq!(settings.host_name, host);
        prop_assert_eq!(settings.user, Some(user));
        prop_assert_eq!(settings.port, port);
    }
}
