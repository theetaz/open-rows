// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SecureField, TextField } from '@openrows/ui';
import type { ConnectionForm } from './useConnectionForm';
import styles from './ConnectionSheet.module.css';

/**
 * Password, always dotted until revealed. A saved password is never sent to
 * the webview, so it shows as "Saved in Keychain" with Change and Forget.
 */
export function PasswordField({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft } = form;
  const [changing, setChanging] = useState(false);
  const saved = draft.hasStoredPassword && draft.passwordMode === 'keep';
  if (saved && !changing)
    return <SavedPassword form={form} change={setChanging} />;
  if (draft.passwordMode === 'clear') return <ForgottenPassword form={form} />;
  return (
    <div className={styles.stack}>
      <SecureField
        className={styles.row}
        label="Password"
        autoFocus={changing}
        placeholder={draft.hasStoredPassword ? 'New password' : 'Optional'}
        value={draft.password}
        onChange={(event) => {
          form.setPassword(event.target.value);
        }}
      />
      {draft.password && (
        <small className={styles.hint}>
          Stored in the macOS Keychain on Save, never in the profile file.
        </small>
      )}
    </div>
  );
}

function SavedPassword(props: {
  form: ConnectionForm;
  change: (changing: boolean) => void;
}): React.JSX.Element {
  return (
    <div className={styles.stack}>
      <TextField
        className={styles.row}
        label="Password"
        readOnly
        value=""
        placeholder="Saved in Keychain"
        onCopy={(event) => {
          event.preventDefault();
        }}
      />
      <span className={styles.hint}>
        <Button
          size="sm"
          onClick={() => {
            props.change(true);
          }}
        >
          Change…
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.form.update({ passwordMode: 'clear', password: '' });
          }}
        >
          Forget saved password
        </Button>
      </span>
    </div>
  );
}

function ForgottenPassword({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  return (
    <div className={styles.stack}>
      <TextField
        className={styles.row}
        label="Password"
        readOnly
        value=""
        placeholder="Removed from Keychain on Save"
      />
      <span className={styles.hint}>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            form.update({ passwordMode: 'keep' });
          }}
        >
          Undo
        </Button>
      </span>
    </div>
  );
}
