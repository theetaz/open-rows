// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use crate::{WireTriggerEvent, WireTriggerTiming};
use serde::{Deserialize, Serialize};
/// Transport representation of `TriggerDetail`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireTriggerDetail {
    /// Trigger name.
    pub name: String,
    /// When it fires.
    pub timing: WireTriggerTiming,
    /// Events it fires on.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub events: Vec<WireTriggerEvent>,
    /// `FOR EACH ROW` (otherwise statement level).
    pub row_level: bool,
    /// Schema-qualified trigger function.
    pub function: String,
    /// Not disabled (`tgenabled <> 'D'`).
    pub enabled: bool,
    /// `CREATE TRIGGER …` as the server prints it.
    pub definition: String,
}
impl From<openrows_driver_api::schema::TriggerDetail> for WireTriggerDetail {
    fn from(value: openrows_driver_api::schema::TriggerDetail) -> Self {
        Self {
            name: value.name,
            timing: value.timing.into(),
            events: value.events.into_iter().map(Into::into).collect(),
            row_level: value.row_level,
            function: value.function,
            enabled: value.enabled,
            definition: value.definition,
        }
    }
}
impl From<WireTriggerDetail> for openrows_driver_api::schema::TriggerDetail {
    fn from(value: WireTriggerDetail) -> Self {
        Self {
            name: value.name,
            timing: value.timing.into(),
            events: value.events.into_iter().map(Into::into).collect(),
            row_level: value.row_level,
            function: value.function,
            enabled: value.enabled,
            definition: value.definition,
        }
    }
}
