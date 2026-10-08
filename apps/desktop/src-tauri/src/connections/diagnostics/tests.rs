// SPDX-License-Identifier: Apache-2.0
use openrows_secrets::{SecretError, SecretKey, SecretStore, SecretString};
use openrows_store::profile::ProfileFiles;

use super::super::dto::{EnvKindDto, PasswordInput, tests::input};
use super::super::route_dto::{SecretInput, SshAuthDto, SshInput, TlsFilesInput, tests::ssh_input};
use super::*;

/// Fails every call, so any secret store access fails the test.
struct Untouchable;

impl SecretStore for Untouchable {
    fn set(&self, _: &SecretKey, _: &SecretString) -> Result<(), SecretError> {
        Err(SecretError::Poisoned)
    }

    fn get(&self, _: &SecretKey) -> Result<Option<SecretString>, SecretError> {
        Err(SecretError::Poisoned)
    }

    fn delete(&self, _: &SecretKey) -> Result<(), SecretError> {
        Err(SecretError::Poisoned)
    }
}

fn untouchable(dir: &tempfile::TempDir) -> ProfileService {
    let files = ProfileFiles::new(dir.path());
    ProfileService::new(files, Arc::new(Untouchable))
}

#[test]
fn testing_without_stored_secrets_never_reads_the_keychain()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let service = untouchable(&dir);
    let saved = super::super::profiles::tests::create_passwordless(&service)?;
    let mut form = input(EnvKindDto::Local);
    form.ssh = Some(ssh_input(SshAuthDto::Password));
    let kept = plan(&service, Some(&saved), &form)?;
    assert!(kept.driver.password.is_none());
    assert!(kept.ssh.password.is_none());
    form.password = PasswordInput::Set {
        value: "typed-synthetic".to_owned(),
    };
    form.ssh = Some(SshInput {
        password: SecretInput::Set {
            value: "ssh-synthetic".to_owned(),
        },
        ..ssh_input(SshAuthDto::Password)
    });
    for id in [None, Some(saved.as_str())] {
        let typed = plan(&service, id, &form)?;
        let password = typed.driver.password.as_ref().map(SecretString::expose);
        assert_eq!(password, Some("typed-synthetic"));
        let ssh = typed.ssh.password.as_ref().map(SecretString::expose);
        assert_eq!(ssh, Some("ssh-synthetic"));
    }
    Ok(())
}

#[test]
fn secrets_the_settings_do_not_use_are_ignored() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let service = untouchable(&dir);
    let mut form = input(EnvKindDto::Local);
    form.ssh = Some(SshInput {
        passphrase: SecretInput::Set {
            value: "unused-synthetic".to_owned(),
        },
        ..ssh_input(SshAuthDto::Agent)
    });
    form.tls_files = TlsFilesInput {
        key_passphrase: SecretInput::Set {
            value: "unused-synthetic".to_owned(),
        },
        ..TlsFilesInput::default()
    };
    let plan = plan(&service, None, &form)?;
    assert!(
        plan.ssh.passphrase.is_none(),
        "agent auth has no key passphrase"
    );
    assert!(
        plan.driver.tls_key_passphrase.is_none(),
        "no client key is set"
    );
    Ok(())
}

#[test]
fn invalid_route_settings_are_rejected_before_connecting() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let service = untouchable(&dir);
    let mut form = input(EnvKindDto::Local);
    form.ssh = Some(SshInput {
        host: "  ".to_owned(),
        ..ssh_input(SshAuthDto::Agent)
    });
    let error = plan(&service, None, &form).err().ok_or("accepted")?;
    assert_eq!(error.message, "SSH host must not be empty");
    form.ssh = Some(ssh_input(SshAuthDto::PrivateKey {
        key_file: " ".to_owned(),
    }));
    let error = plan(&service, None, &form).err().ok_or("accepted")?;
    assert_eq!(error.message, "SSH key file must not be empty");
    Ok(())
}
