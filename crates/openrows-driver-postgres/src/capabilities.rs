// SPDX-License-Identifier: Apache-2.0
//! What the `PostgreSQL` driver supports (docs/05 §2.1).
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Capabilities, Feature, Location};

/// Cancel requests, bound parameters, the full catalog, guarded edits,
/// generated structure changes, transactional DDL, TLS, routes, startup
/// commands, a catalog fingerprint, the process list and `COPY` bulk
/// loading: every feature.
pub(crate) const CAPABILITIES: Capabilities = Capabilities {
    location: Location::Server,
    features: &Feature::ALL,
    object_kinds: &ObjectKind::ALL,
};
