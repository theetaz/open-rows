// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import { Button, Icon, SegmentedControl } from '@openrows/ui';
import type { ProviderConfig } from '@openrows/ipc';
import { AiPreferences } from './AiPreferences';
import { aiSettings } from './aiSettings';
import { removeProvider, saveAiSettings } from './aiActions';
import { ProviderForm } from './ProviderForm';
import { ProviderRow } from './ProviderRow';
import styles from './Ai.module.css';
import { SharingDisclosure } from './SharingDisclosure';
import { t } from '../messages/lazyAi';
import { setAiPage, useAiPage } from './aiPage';
import type { AiPage } from './aiPage';
const useAiSettings = aiSettings.useValue;
const AppsPage = lazy(() => import('../../apps/AppsPage'));

/** Settings → AI: providers (with MCP and sharing) and the agent Apps page. */
export default function AiPane(): React.JSX.Element {
  const page = useAiPage();
  const pages: readonly { value: AiPage; label: string }[] = [
    { value: 'providers', label: t('providersPage') },
    { value: 'apps', label: t('appsPage') },
  ];
  return (
    <section aria-label={t('aiSettings')} className={styles.pane}>
      <SegmentedControl
        aria-label={t('aiPages')}
        className={styles.pages}
        options={pages}
        value={page}
        onValueChange={setAiPage}
      />
      {page === 'apps' ? (
        <Suspense fallback={<p role="status">{t('loadingApps')}</p>}>
          <AppsPage
            onProviders={() => {
              setAiPage('providers');
            }}
          />
        </Suspense>
      ) : (
        <ProvidersPage />
      )}
    </section>
  );
}

/** Lazy-loaded provider setup; AI network permission starts off until explicitly enabled. */
function ProvidersPage(): React.JSX.Element {
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
        error instanceof Error ? error.message : t('aiSettingsUnavailable'),
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className={styles.pane}>
      <SharingDisclosure />
      <div>
        <AiPreferences busy={busy} run={run} />
      </div>
      <div>
        <div className={styles.sectionHead}>
          <h3>{t('aiProviders')}</h3>
          <Button
            size="sm"
            disabled={busy || editing !== undefined}
            onClick={() => {
              setEditing(null);
            }}
          >
            <Icon name="plus" />
            {t('addProvider')}
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
        <ProviderList
          busy={busy}
          editing={editing !== undefined}
          run={run}
          edit={setEditing}
        />
        {failure && <p role="alert">{failure}</p>}
        {settings.providers.length === 0 && (
          <p className={styles.foot}>{t('noProvidersConfigured')}</p>
        )}
      </div>
    </div>
  );
}

/** Saved providers as one grouped list; actions run through the pane's `run`. */
function ProviderList(props: {
  busy: boolean;
  editing: boolean;
  run: (work: () => Promise<void>) => Promise<void>;
  edit: (config: ProviderConfig) => void;
}): React.JSX.Element | null {
  const settings = useAiSettings();
  const { busy, run } = props;
  if (settings.providers.length === 0) return null;
  return (
    <div className={styles.group} aria-busy={busy}>
      {settings.providers.map((config) => (
        <ProviderRow
          key={JSON.stringify(config)}
          config={config}
          enabled={settings.enabled && !busy}
          disabled={busy || props.editing}
          selected={settings.defaultProvider === config.id}
          onEdit={() => {
            props.edit(config);
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
  );
}
