// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Badge } from '@openrows/ui';
import { getAiSharing } from '@openrows/ipc';
import type { SharingLevel, TokenScope } from '@openrows/ipc';
import { useProfiles } from '../connections/useProfiles';
import { aiSettings } from '../settings/ai/aiSettings';
import styles from './Apps.module.css';
import { t } from './messages';

type Sharing = { level: SharingLevel; allowed: boolean } | null;

const LEVELS: Record<SharingLevel, () => string> = {
  none: () => t('levelNone'),
  schemaOnly: () => t('levelSchemaOnly'),
  schemaAndStats: () => t('levelSchemaAndStats'),
  sampleRows: () => t('levelSampleRows'),
};

/** Each scoped connection's saved sharing level for this token's provider. */
function useSharing(scope: TokenScope): Record<string, Sharing> {
  const [sharing, setSharing] = useState<Record<string, Sharing>>({});
  useEffect(() => {
    let alive = true;
    for (const id of scope.connections)
      getAiSharing(id).then(
        (policy) => {
          if (alive)
            setSharing((current) => ({
              ...current,
              [id]: {
                level: policy.level,
                allowed: policy.providers.includes(scope.provider),
              },
            }));
        },
        () => {
          if (alive) setSharing((current) => ({ ...current, [id]: null }));
        },
      );
    return () => {
      alive = false;
    };
  }, [scope]);
  return sharing;
}

function SharingBadge({
  sharing,
}: {
  sharing: Sharing | undefined;
}): React.JSX.Element {
  if (!sharing) return <Badge>{t('sharingUnknown')}</Badge>;
  const shared = sharing.allowed && sharing.level !== 'none';
  return (
    <Badge tone={shared ? 'success' : 'neutral'}>
      {shared ? LEVELS[sharing.level]() : t('levelNone')}
    </Badge>
  );
}

/** Provider identity, expiry and per-connection sharing from existing policy. */
export function AppScope({ scope }: { scope: TokenScope }): React.JSX.Element {
  const { providers } = aiSettings.useValue();
  const { profiles } = useProfiles();
  const sharing = useSharing(scope);
  const provider = providers.find((item) => item.id === scope.provider);
  return (
    <dl className={styles.facts}>
      <dt>{t('provider')}</dt>
      <dd>{provider?.name ?? scope.provider}</dd>
      <dt>{t('expires')}</dt>
      <dd>{new Date(scope.expires * 1000).toLocaleString()}</dd>
      <dt>{t('sharing')}</dt>
      <dd>
        <ul className={styles.sharing}>
          {scope.connections.map((id) => (
            <li key={id}>
              <span>{profiles.find((item) => item.id === id)?.name ?? id}</span>
              <SharingBadge sharing={sharing[id]} />
            </li>
          ))}
        </ul>
      </dd>
    </dl>
  );
}
