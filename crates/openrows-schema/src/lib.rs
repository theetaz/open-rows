// SPDX-License-Identifier: Apache-2.0
//! Schema cache for `OpenRows` (docs/10 §1, M1-16).
//!
//! - [`Scope`]: one lazily read listing of a connection database — its
//!   server's databases, its schemas, one schema's objects of some kinds, or
//!   the bounded name index.
//! - [`SchemaCache`]: listings per [`DatabaseKey`] and scope, each fresh
//!   (read live since the last invalidation) or stale (loaded from a
//!   persisted snapshot, or invalidated by a reconnect or DDL). Stale
//!   listings are served at once and the caller refreshes them in the
//!   background. Snapshots live in the local store's `schema_snapshots`.
//! - [`NameIndex`]: prefix and substring search over qualified names for
//!   the sidebar search, quick open and SQL completion.
//!
//! Everything here is synchronous and free of driver I/O; the app runs the
//! catalog reads on its metadata sessions.

#![forbid(unsafe_code)]

mod cache;
mod codec;
mod index;
mod listing;
mod scope;

pub use cache::{Cached, DatabaseKey, ReadToken, SchemaCache, SnapshotError, save_snapshot};
pub use index::{NameIndex, SearchHits, SearchQuery};
pub use listing::Listing;
pub use scope::Scope;
