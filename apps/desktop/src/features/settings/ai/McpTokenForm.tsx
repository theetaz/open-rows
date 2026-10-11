// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SelectField } from '@openrows/ui';
import type { TokenScope } from '@openrows/ipc';
import { useProfiles } from '../../connections/useProfiles';
import { aiSettings } from './aiSettings';
import styles from './Ai.module.css';
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
    <fieldset disabled={props.busy}>
      <legend>Create read-only token</legend>
      <SelectField
        label="MCP provider identity"
        value={provider}
        options={[
          { value: '', label: 'Choose saved provider' },
          ...providers.map((item) => ({ value: item.id, label: item.name })),
        ]}
        onValueChange={setProvider}
      />
      <p>
        The connection allowlist must permit this provider. Local-only sharing
        is refused for external clients.
      </p>
      {profiles.map((profile) => (
        <label key={profile.id} className={styles.heading}>
          <span>{profile.name}</span>
          <input
            type="checkbox"
            aria-label={`Expose ${profile.name}`}
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
      <SelectField
        label="Token expiry"
        value={hours}
        options={[
          { value: '1', label: '1 hour' },
          { value: '24', label: '1 day' },
          { value: '168', label: '7 days' },
        ]}
        onValueChange={setHours}
      />
      <Button
        disabled={!provider || !connections.length}
        onClick={() => {
          props.create({
            provider,
            connections,
            expires: Math.floor(Date.now() / 1000) + Number(hours) * 3600,
          });
        }}
      >
        Create token
      </Button>
    </fieldset>
  );
}
