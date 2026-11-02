// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button } from '@openrows/ui';
import { promptAiKey } from '@openrows/ipc';
import type { ProviderConfig } from '@openrows/ipc';
const OUTCOME = {
  saved: 'API key saved in the keychain.',
  cancelled: 'No key entered; the stored key is unchanged.',
} as const;
/** Replace a provider's key through the native secure dialog; keyless providers have none. */
export function ProviderKey(props: {
  config: ProviderConfig;
  busy: boolean;
  disabled: boolean;
}): React.JSX.Element | null {
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const ask = async (): Promise<void> => {
    setBusy(true);
    setMessage('');
    try {
      setMessage(OUTCOME[await promptAiKey(props.config.id)]);
    } catch {
      setMessage('The API key could not be stored.');
    } finally {
      setBusy(false);
    }
  };
  if (!props.config.secretRef) return null;
  return (
    <>
      <Button
        size="sm"
        disabled={busy || props.busy || props.disabled}
        onClick={() => {
          void ask();
        }}
      >
        {busy ? 'Waiting for key…' : 'Set API key…'}
      </Button>
      {message && <span role="status">{message}</span>}
    </>
  );
}
