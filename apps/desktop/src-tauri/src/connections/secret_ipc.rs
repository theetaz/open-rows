// SPDX-License-Identifier: Apache-2.0
//! A secret as a JSON string at the IPC boundary. On our side of the boundary
//! the value lives in a zeroizing [`SecretString`]; the webview's copy and the
//! JSON buffers Tauri and serde own are outside our reach (docs/12 §3).
//!
//! A field of this type needs `#[specta(type = String)]`, which keeps the
//! generated bindings a plain `string` (`#[serde(with)]` would split the type).
use openrows_secrets::SecretString;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A secret that serde can read from and write to a JSON string. `Debug`
/// redacts it, like the [`SecretString`] inside.
#[derive(Clone, Debug)]
pub(crate) struct IpcSecret(SecretString);

impl IpcSecret {
    pub(crate) fn expose(&self) -> &str {
        self.0.expose()
    }

    pub(crate) fn secret(&self) -> SecretString {
        self.0.clone()
    }
}

impl From<SecretString> for IpcSecret {
    fn from(secret: SecretString) -> Self {
        Self(secret)
    }
}

impl From<&str> for IpcSecret {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl Serialize for IpcSecret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for IpcSecret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|value| Self(SecretString::new(value)))
    }
}
