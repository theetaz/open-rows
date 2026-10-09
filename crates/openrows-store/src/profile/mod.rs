// SPDX-License-Identifier: Apache-2.0
//! Connection profiles for `PostgreSQL`, `MySQL`, `MariaDB`, SQL Server and `SQLite`
//! (FR-CONN-01/02/03/06/07).
//!
//! Profiles are personal, secret-free JSON files under `connections/`
//! (docs/15 §3); passwords go to a [`openrows_secrets::SecretStore`] and the
//! file records only that a keychain item exists. TLS files and an optional
//! SSH tunnel are engine-agnostic sub-structs (D-10, D-11); their passwords
//! and passphrases are keychain items too ([`SecretSlot`]).

mod attached;
mod conn_url;
mod env;
mod error;
mod files;
mod limits;
mod model;
mod network;
mod params;
mod proxy;
mod search;
mod secrets;
mod service;
mod sqlserver_url;

pub use attached::AttachedFile;
pub use conn_url::{ParsedUrl, parse_url};
pub use env::{EnvKind, EnvTag, Safety};
pub use error::{ProfileError, UrlError};
pub use files::{LoadedProfiles, ProfileFiles};
pub use limits::SessionLimits;
pub use model::{ConnectionProfile, PROFILE_FORMAT, ProfileId, SecretRef, SecretRefs, TlsMode};
pub use network::{CloudCa, Insecurity, SshAuthMethod, SshSettings, TlsSettings, is_local_host};
pub use params::{Driver, FileParams, ServerParams};
pub use proxy::{ProxyKind, ProxySettings};
pub use search::{groups, matches};
pub use secrets::{PasswordStorage, SecretChanges, SecretSlot};
pub use service::{PasswordChange, ProfileService};
