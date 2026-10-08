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

const methods: readonly { value: SshAuthKind; label: string }[] = [
  { value: 'password', label: 'Password' },
  { value: 'privateKey', label: 'Private key' },
  { value: 'agent', label: 'SSH agent' },
  { value: 'keyboardInteractive', label: 'Keyboard-interactive (MFA)' },
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
      label={`SSH · ${sshSummary(ssh)}`}
      defaultOpen={ssh.enabled}
      forceOpen={invalid}
    >
      <div className={styles.section}>
        <div className={styles.switchRow}>
          <Switch
            aria-label="Connect over SSH"
            checked={ssh.enabled}
            onCheckedChange={(enabled) => {
              form.updateSsh({ enabled });
            }}
          />
          <span>
            Connect over SSH
            <small>
              Tunnel through an SSH server; no local port is opened.
            </small>
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
          label="SSH host"
          placeholder="Host or ~/.ssh/config alias"
          {...text('host')}
          {...fieldError(errors.sshHost)}
        />
        <TextField
          label="Port"
          aria-label="SSH port"
          inputMode="numeric"
          placeholder="22"
          className={styles.port}
          {...text('port')}
          {...fieldError(errors.sshPort)}
        />
      </div>
      <TextField
        className={styles.row}
        label="SSH user"
        placeholder="From ~/.ssh/config or your login"
        {...text('user')}
      />
      <SelectField
        className={styles.row}
        label="Auth"
        aria-label="SSH authentication"
        value={ssh.auth}
        options={methods}
        onValueChange={(auth) => {
          updateSsh({ auth });
        }}
      />
      <SshCredentials form={form} ssh={ssh} />
      <TextField
        className={styles.row}
        label="Jump hosts"
        placeholder="Optional: user@bastion:22, next-hop"
        {...text('jumpHosts')}
        {...fieldError(errors.sshJumpHosts)}
      />
      <small className={styles.hint}>
        The database host above is resolved on the SSH server, so localhost
        means the SSH server itself.
      </small>
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
          label="SSH password"
          noun="SSH password"
          secret={ssh.password}
          onChange={(password) => {
            updateSsh({ password });
          }}
        />
      );
    case 'privateKey':
      return (
        <>
          <FileField
            label="Key file"
            kind="sshKey"
            value={ssh.keyFile}
            placeholder="~/.ssh/id_ed25519"
            error={errors.sshKeyFile}
            onChange={(keyFile) => {
              updateSsh({ keyFile });
            }}
          />
          <SecretField
            label="Passphrase"
            noun="key passphrase"
            secret={ssh.passphrase}
            onChange={(passphrase) => {
              updateSsh({ passphrase });
            }}
          />
        </>
      );
    case 'agent':
      return (
        <small className={styles.hint}>
          Uses ssh-agent, then the identity files of a ~/.ssh/config alias.
        </small>
      );
    case 'keyboardInteractive':
      return (
        <small className={styles.hint}>
          OpenRows asks for passwords and one-time codes when connecting.
        </small>
      );
  }
}
