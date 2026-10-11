// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, TextField } from '@openrows/ui';
import type { ProviderConfig } from '@openrows/ipc';
import { isLocalProvider, PROVIDER_KINDS } from './aiSettings';
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
function KeyField(props: {
  value: string;
  local: boolean;
  onChange: (value: string) => void;
}): React.JSX.Element {
  return (
    <TextField
      label={props.local ? 'API key (optional)' : 'API key'}
      type="password"
      autoComplete="off"
      spellCheck={false}
      value={props.value}
      placeholder="Leave empty to keep the stored key"
      onChange={(event) => {
        props.onChange(event.target.value);
      }}
    />
  );
}
/** Transient password field sends keys only to native keychain IPC; cleared on save. */
export function ProviderForm(props: Props): React.JSX.Element {
  const [config, setConfig] = useState(props.initial ?? initialConfig);
  const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const save = async (): Promise<void> => {
    setBusy(true);
    setError('');
    const secretRef =
      key || config.secretRef || !isLocalProvider(config.kind)
        ? `ai/${config.id}/api-key`
        : '';
    const replacement = key;
    setKey('');
    try {
      await saveProvider(
        { ...config, name: config.name.trim(), secretRef },
        replacement,
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
      <KeyField
        value={key}
        local={isLocalProvider(config.kind)}
        onChange={setKey}
      />
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
