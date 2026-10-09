// SPDX-License-Identifier: Apache-2.0
//! Environment tags and the write-safety defaults they imply.

use serde::{Deserialize, Serialize};

/// Environment classification (FR-CONN-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvKind {
    /// The developer's own machine.
    Local,
    /// Shared development.
    Dev,
    /// Pre-production.
    Staging,
    /// Production: confirm-before-write by default (FR-CONN-07).
    Prod,
    /// User-defined label.
    Custom,
}

impl EnvKind {
    /// Lower-case name, as stored in profile files.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Dev => "dev",
            Self::Staging => "staging",
            Self::Prod => "prod",
            Self::Custom => "custom",
        }
    }
}

/// Environment tag with the colour shown in window chrome, tabs and status bar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvTag {
    /// Classification.
    pub kind: EnvKind,
    /// Display label for [`EnvKind::Custom`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// `#rrggbb` colour.
    pub color: String,
}

impl EnvTag {
    /// Tag `kind` with its default colour.
    #[must_use]
    pub fn new(kind: EnvKind) -> Self {
        let color = match kind {
            EnvKind::Local => "#6b7280",
            EnvKind::Dev => "#16a34a",
            EnvKind::Staging => "#d97706",
            EnvKind::Prod => "#dc2626",
            EnvKind::Custom => "#7c3aed",
        };
        Self {
            kind,
            label: None,
            color: color.to_owned(),
        }
    }

    /// A custom tag with its own label and colour.
    #[must_use]
    pub fn custom(label: &str, color: &str) -> Self {
        Self {
            kind: EnvKind::Custom,
            label: Some(label.to_owned()),
            color: color.to_owned(),
        }
    }
}

/// Write-safety switches (FR-CONN-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Safety {
    /// Reject every write statement.
    pub read_only: bool,
    /// Ask before running any write statement.
    pub confirm_writes: bool,
}

impl Safety {
    /// Defaults for an environment: prod confirms writes, others do not.
    #[must_use]
    pub fn default_for(kind: EnvKind) -> Self {
        Self {
            read_only: false,
            confirm_writes: kind == EnvKind::Prod,
        }
    }
}
