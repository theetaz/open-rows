// SPDX-License-Identifier: Apache-2.0
//! Dedicated bounded facet DTOs; native API layouts never cross the transport.
use serde::{Deserialize, Serialize};
/// Transport representation of `TriggerEvent`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WireTriggerEvent {
    /// `INSERT`.
    Insert,
    /// `UPDATE`.
    Update,
    /// `DELETE`.
    Delete,
    /// `TRUNCATE`.
    Truncate,
}
impl From<openrows_driver_api::schema::TriggerEvent> for WireTriggerEvent {
    fn from(value: openrows_driver_api::schema::TriggerEvent) -> Self {
        match value {
            openrows_driver_api::schema::TriggerEvent::Insert => Self::Insert,
            openrows_driver_api::schema::TriggerEvent::Update => Self::Update,
            openrows_driver_api::schema::TriggerEvent::Delete => Self::Delete,
            openrows_driver_api::schema::TriggerEvent::Truncate => Self::Truncate,
        }
    }
}
impl From<WireTriggerEvent> for openrows_driver_api::schema::TriggerEvent {
    fn from(value: WireTriggerEvent) -> Self {
        match value {
            WireTriggerEvent::Insert => Self::Insert,
            WireTriggerEvent::Update => Self::Update,
            WireTriggerEvent::Delete => Self::Delete,
            WireTriggerEvent::Truncate => Self::Truncate,
        }
    }
}
