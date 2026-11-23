// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField } from '@openrows/ui';
import type { TokenScope } from '@openrows/ipc';
import { useProfiles } from '../../connections/useProfiles';
import { aiSettings } from './aiSettings';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
const useAiSettings = aiSettings.useValue;
/** Human-selected provider, connection set and bounded expiry for read-only access. */
export function McpTokenForm(props: {
  busy: boolean;
  create: (scope: TokenScope) => void;
}): React.JSX.Element {
  const { profiles } = useProfiles();
  const { providers } = useAiSettings();
  const [provider, setProvider] = useState('');
  const [connections, setConnections] = useState<string[]>([]);
  const [hours, setHours] = useState('24');
  return (
    <fieldset disabled={props.busy} className={styles.fieldset}>
      <legend>{t('createReadOnly')}</legend>
      <div className={styles.group}>
        <div className={styles.row}>
          <SelectField
            className={styles.inlineField}
            controlSize="compact"
            label={t('mcpProviderIdentity')}
            value={provider}
            options={[
              { value: '', label: t('chooseSavedProvider') },
              ...providers.map((item) => ({
                value: item.id,
                label: item.name,
              })),
            ]}
            onValueChange={setProvider}
          />
        </div>
        {profiles.map((profile) => (
          <label key={profile.id} className={styles.row}>
            <span className={styles.rowBody}>{profile.name}</span>
            <input
              type="checkbox"
              aria-label={t('exposeName', { name: profile.name })}
              checked={connections.includes(profile.id)}
              onChange={(event) => {
                setConnections(
                  event.target.checked
                    ? [...connections, profile.id]
                    : connections.filter((id) => id !== profile.id),
                );
              }}
            />
          </label>
        ))}
        <div className={styles.row}>
          <SelectField
            className={styles.inlineField}
            controlSize="compact"
            label={t('tokenExpiry')}
            value={hours}
            options={[
              { value: '1', label: t('n1Hour') },
              { value: '24', label: t('n1Day') },
              { value: '168', label: t('n7Days') },
            ]}
            onValueChange={setHours}
          />
        </div>
      </div>
      <p className={styles.foot}>{t('connectionAllowlistMust')}</p>
      <div className={styles.footActions}>
        <Button
          size="sm"
          disabled={!provider || !connections.length}
          onClick={() => {
            props.create({
              provider,
              connections,
              expires: Math.floor(Date.now() / 1000) + Number(hours) * 3600,
            });
          }}
        >
          {t('createToken')}
        </Button>
      </div>
    </fieldset>
  );
}
