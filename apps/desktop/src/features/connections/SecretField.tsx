// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SecureField, TextField } from '@openrows/ui';
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
};

/**
 * A keychain secret, always dotted until revealed. A saved secret is never
 * sent to the webview, so it shows as "Saved in Keychain" with Change and
 * Forget; typing replaces it on Save.
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

function SavedSecret(props: Props & { change: () => void }): React.JSX.Element {
  return (
    <div className={styles.stack}>
      <TextField
        className={styles.row}
        label={props.label}
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
          aria-label={`Change ${props.noun}`}
          onClick={props.change}
        >
          Change…
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            props.onChange({ ...props.secret, value: '', mode: 'clear' });
          }}
        >
          {`Forget saved ${props.noun}`}
        </Button>
      </span>
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
