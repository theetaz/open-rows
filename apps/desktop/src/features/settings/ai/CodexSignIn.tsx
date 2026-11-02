// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import { cancelAiVerification, signInCodex } from '@openrows/ipc';
import styles from './Ai.module.css';
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
      if (status.state !== 'signedIn')
        setFailure('Codex did not report a signed-in account.');
      onDone();
    } catch {
      setFailure('Sign-in was cancelled, timed out or failed in Codex.');
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
          Sign in with ChatGPT
        </Button>
        {busy && (
          <Button
            size="sm"
            onClick={() => {
              void cancelAiVerification(props.id).catch(() => undefined);
            }}
          >
            Cancel sign-in
          </Button>
        )}
      </div>
      {busy && (
        <p role="status">
          {url
            ? 'Finish signing in in your browser. If it did not open, visit:'
            : 'Starting Codex sign-in…'}
          {url && <code className={styles.link}>{url}</code>}
        </p>
      )}
      {failure && <p role="alert">{failure}</p>}
    </div>
  );
}
