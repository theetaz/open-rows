// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { ProviderConfig } from '@openrows/ipc';
import { isCliProvider, isLocalProvider, PROVIDER_KINDS } from './aiSettings';
import { ProviderFields } from './ProviderFields';
import { saveProvider } from './aiActions';
import styles from './Ai.module.css';

type Props = {
  initial: ProviderConfig | null;
  onSaved: () => void;
  onCancel: () => void;
};
function initialConfig(): ProviderConfig {
  return {
    id: crypto.randomUUID(),
    name: '',
    kind: 'anthropic',
    baseUrl: PROVIDER_KINDS[0]?.url ?? '',
    allowInsecureRemote: false,
    secretRef: '',
  };
}
/** The key itself is typed into a native dialog after saving, never here. */
function KeyChoice(props: {
  checked: boolean;
  onChange: (value: boolean) => void;
}): React.JSX.Element {
  return (
    <>
      <label>
        <input
          type="checkbox"
          checked={props.checked}
          onChange={(event) => {
            props.onChange(event.target.checked);
          }}
        />{' '}
        Enter an API key after saving
      </label>
      <p className={styles.notice}>
        The key is typed into a secure system dialog and goes straight to the OS
        keychain. This window never receives it.
      </p>
    </>
  );
}
/** Provider settings; keys are entered only in the native dialog. */
export function ProviderForm(props: Props): React.JSX.Element {
  const [config, setConfig] = useState(props.initial ?? initialConfig);
  const [askKey, setAskKey] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  // New remote providers ask for a key by default; edits keep the stored one.
  const ask =
    askKey ?? (props.initial === null && !isLocalProvider(config.kind));
  const save = async (): Promise<void> => {
    setBusy(true);
    setError('');
    const cli = isCliProvider(config.kind);
    const secretRef =
      !cli && (ask || config.secretRef || !isLocalProvider(config.kind))
        ? `ai/${config.id}/api-key`
        : '';
    try {
      await saveProvider(
        { ...config, name: config.name.trim(), secretRef },
        ask && !cli,
      );
      props.onSaved();
    } catch (failure) {
      setError(
        failure instanceof Error
          ? failure.message
          : 'Provider could not be saved.',
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <form
      aria-label={props.initial ? 'Edit AI provider' : 'Add AI provider'}
      className={styles.form}
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
    >
      <ProviderFields config={config} onChange={setConfig} />
      {!isCliProvider(config.kind) && (
        <KeyChoice checked={ask} onChange={setAskKey} />
      )}
      {error && <p role="alert">{error}</p>}
      <div className={styles.actions}>
        <Button type="submit" disabled={busy}>
          {busy ? 'Saving…' : 'Save provider'}
        </Button>
        <Button type="button" disabled={busy} onClick={props.onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  );
}
