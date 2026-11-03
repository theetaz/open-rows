// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextArea, TextField } from '@openrows/ui';
import { cancelCredential, testCredential } from '@openrows/ipc';
import type { ConnectionForm } from './useConnectionForm';
import { commandErrors } from './credentialForm';
import { fieldError, toInput } from './form';
import styles from './ConnectionSheet.module.css';
import { t } from './messages/profile';
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
      setResult(t('commandSucceededPassword'));
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
        label={t('program')}
        value={draft.commandProgram}
        {...fieldError(form.errors.commandProgram)}
        onChange={(event) => {
          form.update({ commandProgram: event.target.value });
        }}
      />
      <label className={styles.stacked}>
        <span>{t('argumentsJsonArray')}</span>
        <TextArea
          aria-label={t('commandArguments')}
          value={draft.commandArgs}
          onChange={(event) => {
            form.update({ commandArgs: event.target.value });
          }}
        />
      </label>
      <TextField
        className={styles.row}
        label={t('timeoutSeconds')}
        inputMode="numeric"
        value={draft.commandTimeout}
        {...fieldError(form.errors.commandTimeout)}
        onChange={(event) => {
          form.update({ commandTimeout: event.target.value });
        }}
      />
      <small className={styles.hint}>{t('runsDirectlyPermissions')}</small>
      <Button
        disabled={busy || Object.keys(commandErrors(draft)).length > 0}
        onClick={() => void run()}
      >
        {t('testCommand')}
      </Button>
      {busy && form.profileId && (
        <Button onClick={() => void cancelCredential(form.profileId ?? '')}>
          {t('cancelCommand')}
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
