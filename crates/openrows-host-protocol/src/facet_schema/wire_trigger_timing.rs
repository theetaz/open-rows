// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `TriggerTiming`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WireTriggerTiming {
    /// `BEFORE`.
    Before,
    /// `AFTER`.
    After,
    /// `INSTEAD OF`.
    InsteadOf,
}
impl From<openrows_driver_api::schema::TriggerTiming> for WireTriggerTiming {
    fn from(value: openrows_driver_api::schema::TriggerTiming) -> Self {
        match value {
            openrows_driver_api::schema::TriggerTiming::Before => Self::Before,
            openrows_driver_api::schema::TriggerTiming::After => Self::After,
            openrows_driver_api::schema::TriggerTiming::InsteadOf => Self::InsteadOf,
        }
    }
}
impl From<WireTriggerTiming> for openrows_driver_api::schema::TriggerTiming {
    fn from(value: WireTriggerTiming) -> Self {
        match value {
            WireTriggerTiming::Before => Self::Before,
            WireTriggerTiming::After => Self::After,
            WireTriggerTiming::InsteadOf => Self::InsteadOf,
        }
    }
}
