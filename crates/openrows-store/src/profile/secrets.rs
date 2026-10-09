// SPDX-License-Identifier: Apache-2.0
//! The secrets a profile may reference, one keychain item each
//! (`conn/<profileId>/<item>`, docs/12 §3), and the edits applied on save.

use openrows_secrets::SecretKey;
use serde::{Deserialize, Serialize};

use super::{ConnectionProfile, PasswordChange, SecretRef, SecretRefs, SshAuthMethod};

/// Where the database password lives (docs/12 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PasswordStorage {
    /// Saved in the OS keychain; the default, also for profile files
    /// written before the choice existed.
    #[default]
    Keychain,
    /// Never saved: asked on every connect and held in memory only while
    /// the connection is open.
    Ask,
    /// Peer, trust, socket or external authentication; never prompt.
    None,
    /// A locally trusted program supplies an ephemeral password.
    Command,
}

/// A secret of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretSlot {
    /// Database password.
    Password,
    /// SSH password (password authentication).
    SshPassword,
    /// Passphrase of the SSH private key file.
    SshPassphrase,
    /// Passphrase of the TLS client key.
    TlsKeyPassphrase,
    /// Password of the proxy login.
    ProxyPassword,
}

impl SecretSlot {
    /// Every slot.
    pub const ALL: [Self; 5] = [
        Self::Password,
        Self::SshPassword,
        Self::SshPassphrase,
        Self::TlsKeyPassphrase,
        Self::ProxyPassword,
    ];

    /// Last segment of the keychain account.
    #[must_use]
    pub fn item(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::SshPassword => "ssh-password",
            Self::SshPassphrase => "ssh-passphrase",
            Self::TlsKeyPassphrase => "tls-key-passphrase",
            Self::ProxyPassword => "proxy-password",
        }
    }
}

impl SecretRefs {
    /// Where `slot` is stored, if anywhere.
    #[must_use]
    pub fn get(&self, slot: SecretSlot) -> Option<SecretRef> {
        match slot {
            SecretSlot::Password => self.password,
            SecretSlot::SshPassword => self.ssh_password,
            SecretSlot::SshPassphrase => self.ssh_passphrase,
            SecretSlot::TlsKeyPassphrase => self.tls_key_passphrase,
            SecretSlot::ProxyPassword => self.proxy_password,
        }
    }

    /// Record where `slot` is stored.
    pub fn set(&mut self, slot: SecretSlot, value: Option<SecretRef>) {
        let field = match slot {
            SecretSlot::Password => &mut self.password,
            SecretSlot::SshPassword => &mut self.ssh_password,
            SecretSlot::SshPassphrase => &mut self.ssh_passphrase,
            SecretSlot::TlsKeyPassphrase => &mut self.tls_key_passphrase,
            SecretSlot::ProxyPassword => &mut self.proxy_password,
        };
        *field = value;
    }
}

/// What a save does to each secret; [`PasswordChange::Keep`] by default.
#[derive(Debug, Clone, Default)]
pub struct SecretChanges {
    /// Database password.
    pub password: PasswordChange,
    /// SSH password.
    pub ssh_password: PasswordChange,
    /// SSH key passphrase.
    pub ssh_passphrase: PasswordChange,
    /// TLS client key passphrase.
    pub tls_key_passphrase: PasswordChange,
    /// Proxy password.
    pub proxy_password: PasswordChange,
}

impl SecretChanges {
    /// Only a database password change.
    #[must_use]
    pub fn password(change: PasswordChange) -> Self {
        Self {
            password: change,
            ..Self::default()
        }
    }

    /// The change for `slot`.
    #[must_use]
    pub fn get(&self, slot: SecretSlot) -> &PasswordChange {
        match slot {
            SecretSlot::Password => &self.password,
            SecretSlot::SshPassword => &self.ssh_password,
            SecretSlot::SshPassphrase => &self.ssh_passphrase,
            SecretSlot::TlsKeyPassphrase => &self.tls_key_passphrase,
            SecretSlot::ProxyPassword => &self.proxy_password,
        }
    }

    /// The change for `slot`, to edit.
    pub fn get_mut(&mut self, slot: SecretSlot) -> &mut PasswordChange {
        match slot {
            SecretSlot::Password => &mut self.password,
            SecretSlot::SshPassword => &mut self.ssh_password,
            SecretSlot::SshPassphrase => &mut self.ssh_passphrase,
            SecretSlot::TlsKeyPassphrase => &mut self.tls_key_passphrase,
            SecretSlot::ProxyPassword => &mut self.proxy_password,
        }
    }
}

impl ConnectionProfile {
    /// Keychain key of `slot` for this profile.
    #[must_use]
    pub fn secret_key(&self, slot: SecretSlot) -> SecretKey {
        SecretKey::for_profile(&self.id.to_string(), slot.item())
    }

    /// Whether the profile's settings keep `slot` in the secret store;
    /// unused secrets are removed on save (turning SSH off forgets its
    /// password, asking for the password forgets the saved one).
    #[must_use]
    pub fn uses_secret(&self, slot: SecretSlot) -> bool {
        let auth = self.ssh.as_ref().map(|ssh| &ssh.auth);
        match slot {
            SecretSlot::Password => {
                !self.driver.is_file() && self.password_storage == PasswordStorage::Keychain
            }
            SecretSlot::SshPassword => matches!(auth, Some(SshAuthMethod::Password)),
            SecretSlot::SshPassphrase => matches!(auth, Some(SshAuthMethod::PrivateKey { .. })),
            SecretSlot::TlsKeyPassphrase => self.tls.client_key.is_some(),
            SecretSlot::ProxyPassword => self
                .proxy
                .as_ref()
                .is_some_and(|proxy| proxy.user.is_some()),
        }
    }
}
