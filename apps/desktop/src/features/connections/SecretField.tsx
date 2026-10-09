// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SecureField, TextField } from '@openrows/ui';
import { SavedSecret } from './SavedSecret';
import { withSecret } from './routeForm';
import type { SecretDraft } from './routeForm';
import styles from './ConnectionSheet.module.css';

type Props = {
  label: string;
  /** Lower-case name in button labels, e.g. `password` or `SSH passphrase`. */
  noun: string;
  secret: SecretDraft;
  onChange: (secret: SecretDraft) => void;
  error?: string | undefined;
  /** Reads the saved value from the Keychain for an explicit reveal. */
  reveal?: (() => Promise<string>) | undefined;
};

/**
 * A keychain secret, always dotted until revealed. A saved secret reaches
 * the webview only through an explicit reveal; Change types a new one and
 * Forget removes it on Save.
 */
export function SecretField(props: Props): React.JSX.Element {
  const { secret } = props;
  const [changing, setChanging] = useState(false);
  if (secret.stored && secret.mode === 'keep' && !changing)
    return (
      <SavedSecret
        {...props}
        change={() => {
          setChanging(true);
        }}
      />
    );
  if (secret.mode === 'clear') return <ForgottenSecret {...props} />;
  return (
    <div className={styles.stack}>
      <SecureField
        className={styles.row}
        label={props.label}
        autoFocus={changing}
        placeholder={secret.stored ? `New ${props.noun}` : 'Optional'}
        value={secret.value}
        {...(props.error ? { error: props.error } : {})}
        onChange={(event) => {
          props.onChange(withSecret(secret, event.target.value));
        }}
      />
      {secret.value && (
        <small className={styles.hint}>
          Stored in the macOS Keychain on Save, never in the profile file.
        </small>
      )}
    </div>
  );
}

function ForgottenSecret(props: Props): React.JSX.Element {
  return (
    <div className={styles.stack}>
      <TextField
        className={styles.row}
        label={props.label}
        readOnly
        value=""
        placeholder="Removed from Keychain on Save"
      />
      <span className={styles.hint}>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.onChange({ ...props.secret, mode: 'keep' });
          }}
        >
          Undo
        </Button>
      </span>
    </div>
  );
}
