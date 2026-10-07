// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` connection profiles (FR-CONN-01/02/03/06/07).
//!
//! Profiles are personal, secret-free JSON files under `connections/`
//! (docs/15 §3); passwords go to a [`openrows_secrets::SecretStore`] and the
//! file records only that a keychain item exists.

mod env;
mod error;
mod files;
mod model;
mod pg_url;
mod search;
mod service;

pub use env::{EnvKind, EnvTag, Safety};
pub use error::{ProfileError, UrlError};
pub use files::{LoadedProfiles, ProfileFiles};
pub use model::{
    ConnectionProfile, Driver, PROFILE_FORMAT, PgParams, ProfileId, SecretRef, SecretRefs, TlsMode,
    TlsSettings,
};
pub use pg_url::{ParsedUrl, parse_postgres_url};
pub use search::{groups, matches};
pub use service::{PasswordChange, ProfileService};
