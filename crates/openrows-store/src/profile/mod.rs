// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` connection profiles (FR-CONN-01/02/03/06/07).
//!
//! Profiles are personal, secret-free JSON files under `connections/`
//! (docs/15 §3); passwords go to a [`openrows_secrets::SecretStore`] and the
//! file records only that a keychain item exists. TLS files and an optional
//! SSH tunnel are engine-agnostic sub-structs (D-10, D-11); their passwords
//! and passphrases are keychain items too ([`SecretSlot`]).

mod env;
mod error;
mod files;
mod model;
mod network;
mod pg_url;
mod search;
mod secrets;
mod service;

pub use env::{EnvKind, EnvTag, Safety};
pub use error::{ProfileError, UrlError};
pub use files::{LoadedProfiles, ProfileFiles};
pub use model::{
    ConnectionProfile, Driver, PROFILE_FORMAT, PgParams, ProfileId, SecretRef, SecretRefs, TlsMode,
};
pub use network::{SshAuthMethod, SshSettings, TlsSettings, is_local_host};
pub use pg_url::{ParsedUrl, parse_postgres_url};
pub use search::{groups, matches};
pub use secrets::{SecretChanges, SecretSlot};
pub use service::{PasswordChange, ProfileService};
