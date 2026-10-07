// SPDX-License-Identifier: Apache-2.0
//! Webview mirrors of store enums (the store crate stays free of specta).
use openrows_store::profile::{EnvKind, TlsMode};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Environment classification (FR-CONN-06).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EnvKindDto {
    Local,
    Dev,
    Staging,
    Prod,
    Custom,
}

impl From<EnvKind> for EnvKindDto {
    fn from(kind: EnvKind) -> Self {
        match kind {
            EnvKind::Local => Self::Local,
            EnvKind::Dev => Self::Dev,
            EnvKind::Staging => Self::Staging,
            EnvKind::Prod => Self::Prod,
            EnvKind::Custom => Self::Custom,
        }
    }
}

impl From<EnvKindDto> for EnvKind {
    fn from(kind: EnvKindDto) -> Self {
        match kind {
            EnvKindDto::Local => Self::Local,
            EnvKindDto::Dev => Self::Dev,
            EnvKindDto::Staging => Self::Staging,
            EnvKindDto::Prod => Self::Prod,
            EnvKindDto::Custom => Self::Custom,
        }
    }
}

/// libpq `sslmode` values (docs/12 §4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TlsModeDto {
    Disable,
    Allow,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

impl From<TlsMode> for TlsModeDto {
    fn from(mode: TlsMode) -> Self {
        match mode {
            TlsMode::Disable => Self::Disable,
            TlsMode::Allow => Self::Allow,
            TlsMode::Prefer => Self::Prefer,
            TlsMode::Require => Self::Require,
            TlsMode::VerifyCa => Self::VerifyCa,
            TlsMode::VerifyFull => Self::VerifyFull,
        }
    }
}

impl From<TlsModeDto> for TlsMode {
    fn from(mode: TlsModeDto) -> Self {
        match mode {
            TlsModeDto::Disable => Self::Disable,
            TlsModeDto::Allow => Self::Allow,
            TlsModeDto::Prefer => Self::Prefer,
            TlsModeDto::Require => Self::Require,
            TlsModeDto::VerifyCa => Self::VerifyCa,
            TlsModeDto::VerifyFull => Self::VerifyFull,
        }
    }
}
