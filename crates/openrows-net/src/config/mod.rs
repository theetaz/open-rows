// SPDX-License-Identifier: Apache-2.0
//! `~/.ssh/config` subset: `Host`, `HostName`, `User`, `Port`, `IdentityFile`,
//! `IdentitiesOnly`, `ProxyJump` and `Include`.
//!
//! Parsing is pure over a [`ConfigSource`], so it can be fuzzed and tested
//! without touching the real home directory. As in OpenSSH, the first value
//! obtained for a parameter wins and `IdentityFile` accumulates. `Match`
//! blocks are recognised but never applied; other keywords are ignored.

mod jump;
mod load;
mod parse;
mod pattern;

use std::{
    io,
    path::{Path, PathBuf},
};

pub use jump::JumpSpec;
pub(crate) use load::expand_tilde;
pub use load::{ConfigSource, FileSystem};
use load::{Loader, Node};
use parse::{Directive, ProxyJumpValue};
pub(crate) use pattern::pattern_list_matches;

/// Default SSH port.
pub const DEFAULT_PORT: u16 = 22;

/// Failure to load an SSH config file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// A line could not be parsed.
    #[error("{path}:{line}: {message}", path = path.display())]
    Syntax {
        /// File containing the error.
        path: PathBuf,
        /// 1-based line number.
        line: usize,
        /// What is wrong.
        message: String,
    },
    /// A file or directory could not be read.
    #[error("could not read {path}", path = path.display())]
    Io {
        /// File or directory.
        path: PathBuf,
        /// I/O error.
        #[source]
        source: io::Error,
    },
    /// `Include` nesting exceeded the limit (usually an include cycle).
    #[error("Include nesting deeper than {max} in {path}", path = path.display())]
    IncludeDepth {
        /// File whose `Include` exceeded the limit.
        path: PathBuf,
        /// Maximum depth.
        max: usize,
    },
}

/// Effective settings for one host alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostSettings {
    /// The alias that was looked up.
    pub alias: String,
    /// Address to connect to (`HostName`, `%h` expanded; defaults to the alias).
    pub host_name: String,
    /// Login user, if configured.
    pub user: Option<String>,
    /// Port (default 22).
    pub port: u16,
    /// Private key files in order, `~` expanded.
    pub identity_files: Vec<PathBuf>,
    /// Use only the configured identity files, not every agent key.
    pub identities_only: bool,
    /// Jump hosts, first hop first; empty for a direct connection.
    pub proxy_jump: Vec<JumpSpec>,
}

/// A parsed SSH client configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SshConfig {
    nodes: Vec<Node>,
    home: PathBuf,
}

impl SshConfig {
    /// Load `path` from disk; a missing file is an empty configuration.
    ///
    /// # Errors
    /// [`ConfigError`] for unreadable files, syntax errors or include cycles.
    pub fn load(path: &Path, home: &Path) -> Result<Self, ConfigError> {
        match FileSystem.read(path) {
            Ok(text) => Self::parse(&text, path, home, &FileSystem),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self {
                nodes: Vec::new(),
                home: home.to_path_buf(),
            }),
            Err(source) => Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    /// Parse `text` (read from `path`), resolving `Include` through `source`.
    ///
    /// # Errors
    /// [`ConfigError`] for syntax errors, unreadable includes or include cycles.
    pub fn parse(
        text: &str,
        path: &Path,
        home: &Path,
        source: &dyn ConfigSource,
    ) -> Result<Self, ConfigError> {
        let nodes = Loader { home, source }.parse_text(text, path, 0)?;
        Ok(Self {
            nodes,
            home: home.to_path_buf(),
        })
    }

    /// Effective settings for `alias`.
    #[must_use]
    pub fn resolve(&self, alias: &str) -> HostSettings {
        let mut partial = Partial::default();
        let mut active = true;
        partial.apply(&self.nodes, alias, &mut active);
        partial.finish(alias, &self.home)
    }
}

#[derive(Default)]
struct Partial {
    host_name: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_files: Vec<String>,
    identities_only: Option<bool>,
    proxy_jump: Option<ProxyJumpValue>,
}

impl Partial {
    fn apply(&mut self, nodes: &[Node], alias: &str, active: &mut bool) {
        for node in nodes {
            match node {
                Node::Directive(Directive::Host(patterns)) => {
                    *active = pattern_list_matches(patterns, alias);
                }
                Node::Directive(Directive::Match) => *active = false,
                Node::Directive(directive) if *active => self.set(directive),
                // An include inside an active block starts active and restores the state afterwards.
                Node::Include(children) if *active => self.apply(children, alias, &mut true),
                Node::Directive(_) | Node::Include(_) => {}
            }
        }
    }

    fn set(&mut self, directive: &Directive) {
        match directive {
            Directive::HostName(value) => set_once(&mut self.host_name, value),
            Directive::User(value) => set_once(&mut self.user, value),
            Directive::Port(value) => set_once(&mut self.port, value),
            Directive::IdentityFile(value) => self.identity_files.push(value.clone()),
            Directive::IdentitiesOnly(value) => set_once(&mut self.identities_only, value),
            Directive::ProxyJump(value) => set_once(&mut self.proxy_jump, value),
            Directive::Host(_)
            | Directive::Match
            | Directive::Include(_)
            | Directive::Unsupported => {}
        }
    }

    fn finish(self, alias: &str, home: &Path) -> HostSettings {
        let host_name = self
            .host_name
            .map_or_else(|| alias.to_owned(), |name| expand_host_tokens(&name, alias));
        HostSettings {
            alias: alias.to_owned(),
            host_name,
            user: self.user,
            port: self.port.unwrap_or(DEFAULT_PORT),
            identity_files: self
                .identity_files
                .iter()
                .filter(|path| !path.eq_ignore_ascii_case("none"))
                .map(|path| expand_tilde(path, home))
                .collect(),
            identities_only: self.identities_only.unwrap_or(false),
            proxy_jump: match self.proxy_jump {
                Some(ProxyJumpValue::Hops(hops)) => hops,
                Some(ProxyJumpValue::Disabled) | None => Vec::new(),
            },
        }
    }
}

fn set_once<T: Clone>(slot: &mut Option<T>, value: &T) {
    if slot.is_none() {
        *slot = Some(value.clone());
    }
}

/// Expand `%h` (the alias) and `%%` in `HostName`.
fn expand_host_tokens(value: &str, alias: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match (c, chars.clone().next()) {
            ('%', Some('h')) => {
                out.push_str(alias);
                chars.next();
            }
            ('%', Some('%')) => {
                out.push('%');
                chars.next();
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests;
