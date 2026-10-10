// SPDX-License-Identifier: Apache-2.0
//! Strongly typed, time-ordered identifiers for `OpenRows` engine resources.
//!
//! Identifiers use UUID v7 and serialize as canonical hyphenated strings.

pub mod safety;

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::{Uuid, Variant, Version};

/// Failure to parse an `OpenRows` UUID v7 identifier.
#[derive(Debug, thiserror::Error)]
pub enum ParseIdError {
    /// The input is not a UUID.
    #[error("invalid identifier: {0}")]
    InvalidUuid(#[from] uuid::Error),
    /// The UUID does not use the RFC variant and version 7.
    #[error("identifier must be an RFC UUID version 7")]
    NotVersion7,
}

macro_rules! define_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a unique UUID v7, ordered after earlier sequential calls.
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = ParseIdError;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                let uuid = Uuid::parse_str(input)?;
                if uuid.get_version() != Some(Version::SortRand)
                    || uuid.get_variant() != Variant::RFC4122
                {
                    return Err(ParseIdError::NotVersion7);
                }
                Ok(Self(uuid))
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let input = String::deserialize(deserializer)?;
                input.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

define_id!(ProfileId, "Identifier for a saved connection profile.");
define_id!(SessionId, "Identifier for a live database session.");
define_id!(QueryId, "Identifier for a query execution.");
define_id!(ResultId, "Identifier for a query result set.");

#[cfg(test)]
mod tests;
