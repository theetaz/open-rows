// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextArea, TextField } from '@openrows/ui';
import { cancelCredential, testCredential } from '@openrows/ipc';
import type { ConnectionForm } from './useConnectionForm';
import { commandErrors } from './credentialForm';
import { fieldError, toInput } from './form';
import styles from './ConnectionSheet.module.css';
/** Credential invocation fields and an explicit, value-free test result. */
export function CredentialFields({
  form,
}: {
  form: ConnectionForm;
}): React.JSX.Element {
  const { draft } = form;
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const run = async (): Promise<void> => {
    setBusy(true);
    setResult(null);
    try {
      await testCredential(form.profileId, toInput(draft));
      setResult(
        'Command succeeded. A password was produced; its value is hidden.',
      );
    } catch (error) {
      setResult(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className={styles.stack}>
      <TextField
        className={styles.row}
        label="Program"
        value={draft.commandProgram}
        {...fieldError(form.errors.commandProgram)}
        onChange={(event) => {
          form.update({ commandProgram: event.target.value });
        }}
      />
      <label className={styles.stacked}>
        <span>Arguments (JSON array)</span>
        <TextArea
          aria-label="Command arguments"
          value={draft.commandArgs}
          onChange={(event) => {
            form.update({ commandArgs: event.target.value });
          }}
        />
      </label>
      <TextField
        className={styles.row}
        label="Timeout (seconds)"
        inputMode="numeric"
        value={draft.commandTimeout}
        {...fieldError(form.errors.commandTimeout)}
        onChange={(event) => {
          form.update({ commandTimeout: event.target.value });
        }}
      />
      <small className={styles.hint}>
        Runs directly with your permissions. Arguments are literal; use a
        credential provider rather than putting secrets in arguments. Trust is
        local to this machine.
      </small>
      <Button
        disabled={busy || Object.keys(commandErrors(draft)).length > 0}
        onClick={() => void run()}
      >
        Test command
      </Button>
      {busy && form.profileId && (
        <Button onClick={() => void cancelCredential(form.profileId ?? '')}>
          Cancel command
        </Button>
      )}
      {result && <p role="status">{result}</p>}
      {form.errors.commandArgs && (
        <p className={styles.error} role="alert">
          {form.errors.commandArgs}
        </p>
      )}
    </div>
  );
}
