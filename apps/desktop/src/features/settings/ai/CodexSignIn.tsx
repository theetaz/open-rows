// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { cancelAiVerification, signInCodex } from '@openrows/ipc';
import styles from './Ai.module.css';
import { t } from '../messages/lazyAi';
type SignIn = {
  url: string;
  busy: boolean;
  failure: string;
  start: () => Promise<void>;
};
/** Sign-in runs inside the CLI; leaving the row cancels it and stops the CLI. */
function useCodexSignIn(id: string, onDone: () => void): SignIn {
  const [url, setUrl] = useState('');
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState('');
  useEffect(
    () => () => {
      void cancelAiVerification(id).catch(() => undefined);
    },
    [id],
  );
  const start = async (): Promise<void> => {
    setBusy(true);
    setUrl('');
    setFailure('');
    try {
      const status = await signInCodex(id, setUrl);
      if (status.state !== 'signedIn') setFailure(t('codexDidNot'));
      onDone();
    } catch {
      setFailure(t('signWasCancelled'));
    } finally {
      setBusy(false);
    }
  };
  return { url, busy, failure, start };
}
/** Start the Codex CLI's own ChatGPT sign-in in the browser. */
export function CodexSignIn(props: {
  id: string;
  enabled: boolean;
  onDone: () => void;
}): React.JSX.Element {
  const { url, busy, failure, start } = useCodexSignIn(props.id, props.onDone);
  return (
    <div className={styles.details}>
      <div className={styles.actions}>
        <Button
          size="sm"
          disabled={!props.enabled || busy}
          onClick={() => {
            void start();
          }}
        >
          {t('signChatgpt')}
        </Button>
        {busy && (
          <Button
            size="sm"
            onClick={() => {
              void cancelAiVerification(props.id).catch(() => undefined);
            }}
          >
            {t('cancelSign')}
          </Button>
        )}
      </div>
      {busy && (
        <p role="status">
          {url ? t('finishSigningBrowser') : t('startingCodexSign')}
          {url && <code className={styles.link}>{url}</code>}
        </p>
      )}
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
