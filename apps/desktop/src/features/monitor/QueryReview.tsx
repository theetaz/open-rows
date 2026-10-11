// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { Button, Switch } from '@openrows/ui';
import { Sheet } from '@openrows/ui/sheet';
import { useExplain } from '../explain/useExplain';
import { setExplain } from '../explain/explainStore';
import { explainResultTab } from '../explain/ExplainResult';
import styles from './Monitor.module.css';

/** Review redacted SQL, then request plan-only through the existing Explain safety boundary. */
export function QueryReview(props: {
  sql: string;
  sessionId: string | null;
  onClose: () => void;
}): React.JSX.Element {
  const [sql, setSql] = useState(props.sql);
  const [reviewed, setReviewed] = useState(false);
  const [id] = useState(() => crypto.randomUUID());
  const editor = {
    current: { text: () => sql, cursor: () => 0, selection: () => '' },
  };
  const explain = useExplain(id, props.sessionId, editor);
  const cancel = explain.cancel;
  useEffect(
    () => () => {
      cancel();
      setExplain(id, null);
    },
    [cancel, id],
  );
  const result = explainResultTab(explain);
  const close = (): void => {
    explain.close();
    props.onClose();
  };
  return (
    <Sheet title="Review SQL before Explain" onClose={close}>
      <div className={styles.review}>
        <p>
          Collected SQL is normalised and its literals are redacted. Supply a
          complete statement and bindings before requesting a plan. Explain is
          plan-only.
        </p>
        <textarea
          aria-label="Reviewed SQL"
          value={sql}
          onChange={(event) => {
            setSql(event.target.value);
            setReviewed(false);
          }}
        />
        <label>
          Statement and bindings reviewed{' '}
          <Switch
            aria-label="Statement reviewed"
            checked={reviewed}
            onCheckedChange={setReviewed}
          />
        </label>
        <Button
          disabled={
            !reviewed || !props.sessionId || explain.state?.status === 'running'
          }
          onClick={() => {
            explain.run(false);
          }}
        >
          Explain plan only
        </Button>
        {result?.content}
        <Button onClick={close}>Close</Button>
      </div>
    </Sheet>
  );
}
