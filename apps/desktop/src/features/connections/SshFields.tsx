// SPDX-License-Identifier: Apache-2.0
import {
  DisclosureSection,
  SelectField,
  Switch,
  TextField,
} from '@openrows/ui';
import { fieldError } from './form';
import { FileField } from './FileField';
import { SecretField } from './SecretField';
import { sshSummary } from './routeForm';
import type { SshAuthKind, SshDraft } from './routeForm';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';
import { rich, t } from './messages/routes';

const methods: readonly { value: SshAuthKind; label: string }[] = [
  { value: 'password', label: t('password') },
  { value: 'privateKey', label: t('privateKey') },
  { value: 'agent', label: t('sshAgent') },
  { value: 'keyboardInteractive', label: t('keyboardInteractiveMfa') },
];

/**
 * "SSH" disclosure: tunnel the connection through an SSH server, with jump
 * hosts. Collapsed until the profile uses SSH or a field in it is invalid.
 */
export function SshFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { ssh } = form.draft;
  const { errors } = form;
  const invalid = Boolean(
    errors.sshHost ??
    errors.sshPort ??
    errors.sshKeyFile ??
    errors.sshJumpHosts,
  );
  return (
    <DisclosureSection
      label={t('sshSsh', { ssh: sshSummary(ssh) })}
      defaultOpen={ssh.enabled}
      forceOpen={invalid}
    >
      <div className={styles.section}>
        <div className={styles.switchRow}>
          <Switch
            aria-label={t('connectOverSsh')}
            checked={ssh.enabled}
            onCheckedChange={(enabled) => {
              form.updateSsh({ enabled });
            }}
          />
          <span>
            {rich('connectOverSshsmall', {
              small: <small>{t('tunnelThroughSsh')}</small>,
            })}
          </span>
        </div>
        {ssh.enabled && <SshServer form={form} />}
      </div>
    </DisclosureSection>
  );
}

function SshServer({ form }: { form: ConnectionForm }): React.JSX.Element {
  const { ssh } = form.draft;
  const { errors, updateSsh } = form;
  const text = (key: 'host' | 'port' | 'user' | 'jumpHosts') => ({
    value: ssh[key],
    spellCheck: false,
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => {
      updateSsh({ [key]: event.target.value });
    },
  });
  return (
    <>
      <div className={styles.pair}>
        <TextField
          className={styles.row}
          label={t('sshHost')}
          placeholder={t('hostSshConfig')}
          {...text('host')}
          {...fieldError(errors.sshHost)}
        />
        <TextField
          label={t('port')}
          aria-label={t('sshPort')}
          inputMode="numeric"
          placeholder="22"
          className={styles.port}
          {...text('port')}
          {...fieldError(errors.sshPort)}
        />
      </div>
      <TextField
        className={styles.row}
        label={t('sshUser')}
        placeholder={t('sshConfigLogin')}
        {...text('user')}
      />
      <SelectField
        className={styles.row}
        label={t('auth')}
        aria-label={t('sshAuthentication')}
        value={ssh.auth}
        options={methods}
        onValueChange={(auth) => {
          updateSsh({ auth });
        }}
      />
      <SshCredentials form={form} ssh={ssh} />
      <TextField
        className={styles.row}
        label={t('jumpHosts')}
        placeholder={t('optionalUserBastion')}
        {...text('jumpHosts')}
        {...fieldError(errors.sshJumpHosts)}
      />
      <small className={styles.hint}>{t('databaseHostAbove')}</small>
    </>
  );
}

function SshCredentials({
  form,
  ssh,
}: {
  form: ConnectionForm;
  ssh: SshDraft;
}): React.JSX.Element | null {
  const { errors, updateSsh } = form;
  switch (ssh.auth) {
    case 'password':
      return (
        <SecretField
          label={t('sshPassword')}
          noun={t('sshPassword')}
          secret={ssh.password}
          reveal={form.reveal('sshPassword')}
          onChange={(password) => {
            updateSsh({ password });
          }}
        />
      );
    case 'privateKey':
      return (
        <>
          <FileField
            label={t('keyFile')}
            kind="sshKey"
            value={ssh.keyFile}
            placeholder={t('sshIdEd25519')}
            error={errors.sshKeyFile}
            onChange={(keyFile) => {
              updateSsh({ keyFile });
            }}
          />
          <SecretField
            label={t('passphrase')}
            noun={t('keyPassphrase')}
            secret={ssh.passphrase}
            reveal={form.reveal('sshPassphrase')}
            onChange={(passphrase) => {
              updateSsh({ passphrase });
            }}
          />
        </>
      );
    case 'agent':
      return <small className={styles.hint}>{t('usesSshAgent')}</small>;
    case 'keyboardInteractive':
      return (
        <small className={styles.hint}>{t('openrowsAsksPasswords')}</small>
      );
  }
}
