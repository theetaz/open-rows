// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import type { ProviderConfig } from '@openrows/ipc';
import { AiPreferences } from './AiPreferences';
import { aiSettings } from './aiSettings';
import { removeProvider, saveAiSettings } from './aiActions';
import { ProviderForm } from './ProviderForm';
import { ProviderRow } from './ProviderRow';
import styles from './Ai.module.css';
const useAiSettings = aiSettings.useValue;
/** Lazy-loaded provider setup; AI network permission starts off until explicitly enabled. */
export default function AiPane(): React.JSX.Element {
  const settings = useAiSettings();
  const [editing, setEditing] = useState<ProviderConfig | null | undefined>(
    undefined,
  );
  const [failure, setFailure] = useState('');
  const [busy, setBusy] = useState(false);
  const run = async (work: () => Promise<void>): Promise<void> => {
    setBusy(true);
    setFailure('');
    try {
      await work();
    } catch (error) {
      setFailure(
        error instanceof Error ? error.message : 'AI settings unavailable.',
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <section aria-label="AI settings">
      <h3>AI providers</h3>
      <AiPreferences busy={busy} run={run} />
      <div className={styles.actions}>
        <Button
          disabled={busy || editing !== undefined}
          onClick={() => {
            setEditing(null);
          }}
        >
          Add provider
        </Button>
      </div>
      {editing !== undefined && (
        <ProviderForm
          key={editing?.id ?? 'new'}
          initial={editing}
          onSaved={() => {
            setEditing(undefined);
          }}
          onCancel={() => {
            setEditing(undefined);
          }}
        />
      )}
      <div aria-busy={busy}>
        {settings.providers.map((config) => (
          <ProviderRow
            key={JSON.stringify(config)}
            config={config}
            enabled={settings.enabled && !busy}
            disabled={busy || editing !== undefined}
            selected={settings.defaultProvider === config.id}
            onEdit={() => {
              setEditing(config);
            }}
            onRemove={() => {
              void run(() => removeProvider(config));
            }}
            onDefault={() => {
              void run(() =>
                saveAiSettings({
                  ...settings,
                  defaultProvider: config.id,
                  defaultModel: '',
                }),
              );
            }}
          />
        ))}
      </div>
      {failure && <p role="alert">{failure}</p>}
      {settings.providers.length === 0 && (
        <p>No providers configured. Keys are stored only in the OS keychain.</p>
      )}
    </section>
  );
}
