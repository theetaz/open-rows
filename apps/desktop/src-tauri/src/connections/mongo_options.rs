// SPDX-License-Identifier: Apache-2.0
//! Non-secret `MongoDB` options shared by URL, form and saved profiles.
use openrows_store::profile::Driver;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MongoOptionsDto {
    /// `mongodb+srv`: hosts and options come from DNS.
    pub(crate) srv: bool,
    /// Talk only to the given server, without replica-set discovery
    /// (`directConnection=true`); never with SRV.
    pub(crate) direct: bool,
    /// Authentication database; empty uses the profile's database, else `admin`.
    pub(crate) auth_source: String,
}
impl MongoOptionsDto {
    pub(crate) fn of(driver: Driver, options: &BTreeMap<String, String>) -> Option<Self> {
        (driver == Driver::Mongodb).then(|| Self {
            srv: options
                .get("mongodb_srv")
                .is_some_and(|value| value == "true"),
            direct: options
                .get("mongodb_direct")
                .is_some_and(|value| value == "true"),
            auth_source: options.get("authSource").cloned().unwrap_or_default(),
        })
    }
    pub(crate) fn options(&self) -> BTreeMap<String, String> {
        let mut options = BTreeMap::new();
        if self.srv {
            options.insert("mongodb_srv".into(), "true".into());
        } else if self.direct {
            options.insert("mongodb_direct".into(), "true".into());
        }
        if !self.auth_source.trim().is_empty() {
            options.insert("authSource".into(), self.auth_source.trim().into());
        }
        options
    }
}
