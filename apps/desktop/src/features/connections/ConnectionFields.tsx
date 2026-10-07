// SPDX-License-Identifier: Apache-2.0
import { Button, TextField } from '@openrows/ui';
import type { TlsModeDto } from '@openrows/ipc';
import { fieldError } from './form';
import type { Draft } from './form';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

const tlsModes: readonly (readonly [TlsModeDto, string])[] = [
  ['prefer', 'Prefer (no TLS yet)'],
  ['disable', 'Disable'],
  ['require', 'Require'],
  ['verify-full', 'Verify full'],
];

/** Name, group, address, credentials and TLS mode. */
export function ConnectionFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft, errors, update } = form;
  const text = (key: keyof Draft) => ({
    value: String(draft[key]),
    onChange: (event: React.ChangeEvent<HTMLInputElement>) => {
      update({ [key]: event.target.value });
    },
  });
  return (
    <div className={styles.grid}>
      <TextField label="Name" {...text('name')} {...fieldError(errors.name)} />
      <TextField label="Group" placeholder="Optional" {...text('group')} />
      <TextField label="Host" {...text('host')} {...fieldError(errors.host)} />
      <TextField
        label="Port"
        inputMode="numeric"
        {...text('port')}
        {...fieldError(errors.port)}
      />
      <TextField
        label="Database"
        placeholder="Same as user"
        {...text('database')}
      />
      <TextField label="User" {...text('user')} {...fieldError(errors.user)} />
      <PasswordField form={form} />
      <label className={styles.select}>
        <span>TLS</span>
        <select
          value={draft.tlsMode}
          onChange={(event) => {
            update({ tlsMode: event.target.value as TlsModeDto });
          }}
        >
          {tlsModes.map(([mode, label]) => (
            <option key={mode} value={mode}>
              {label}
            </option>
          ))}
          {!tlsModes.some(([mode]) => mode === draft.tlsMode) && (
            <option value={draft.tlsMode}>{draft.tlsMode}</option>
          )}
        </select>
      </label>
    </div>
  );
}

function passwordHint(draft: Draft): string {
  switch (draft.passwordMode) {
    case 'url':
      return 'Password provided in the URL; saved to the keychain on Save.';
    case 'clear':
      return 'The saved password will be removed on Save.';
    case 'set':
      return 'Saved to the OS keychain, never to the profile file.';
    case 'keep':
      return draft.hasStoredPassword
        ? 'Saved in the keychain. Type to replace it.'
        : 'Optional; stored in the OS keychain.';
  }
}

function PasswordField({ form }: { form: ConnectionForm }): React.JSX.Element {
  const { draft } = form;
  const stored = draft.hasStoredPassword && draft.passwordMode === 'keep';
  return (
    <div className={styles.password}>
      <TextField
        label="Password"
        type="password"
        autoComplete="off"
        placeholder={stored ? '••••••••' : ''}
        value={draft.password}
        onChange={(event) => {
          form.setPassword(event.target.value);
        }}
      />
      <small>{passwordHint(draft)}</small>
      {stored && (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            form.update({ passwordMode: 'clear', password: '', url: '' });
          }}
        >
          Forget saved password
        </Button>
      )}
    </div>
  );
}
