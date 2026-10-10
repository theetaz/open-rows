// SPDX-License-Identifier: Apache-2.0
//! Remote HTTP facets and their precise limits (docs/05 §18).
use openrows_driver_api::{Capabilities, Feature, Location, catalog::ObjectKind};
pub(crate) const CAPABILITIES: Capabilities = Capabilities {
    location: Location::Server,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::Catalog,
        Feature::Schemas,
        Feature::RowEdits,
        Feature::TransactionalDdl,
        Feature::Tls,
        Feature::Routes,
        Feature::StartupCommands,
    ],
    object_kinds: &[ObjectKind::Table, ObjectKind::View],
};
