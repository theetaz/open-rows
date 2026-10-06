// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Badge, Button, IconButton } from '@openrows/ui';
import {
  assistantPrompt,
  assistantSql,
  policyChips,
  toolTrace,
} from './data/content';
import { SqlCode } from './SqlCode';
import styles from './Inspector.module.css';

/** Static assistant transcript with local composition and SQL preview actions. */
export function Assistant({
  onQuery,
}: {
  onQuery: (sql: string) => void;
}): React.JSX.Element {
  const [message, setMessage] = useState('');
  const [sent, setSent] = useState('');
  return (
    <div className={styles.assistant}>
      <Button className={styles.provider}>
        <span className={styles.providerTile}>›_</span>
        <span>
          <strong>Claude Code CLI</strong>
          <small>Uses your Claude subscription</small>
        </span>
        <span aria-hidden="true">⌃</span>
      </Button>
      <div className={styles.policies}>
        {policyChips.map((chip) => (
          <Badge key={chip}>{chip}</Badge>
        ))}
      </div>
      <div className={styles.conversation}>
        <p className={styles.bubble}>{assistantPrompt}</p>
        <div className={styles.trace}>
          {toolTrace.map((line) => (
            <span key={line}>
              <span aria-hidden="true">✓ </span>
              {line}
            </span>
          ))}
        </div>
        <p>
          This read-only query uses the <code>orders_status_created_idx</code>{' '}
          index, so it stays fast on 2.4M rows.
        </p>
        <AssistantSql onQuery={onQuery} />
        {sent && <p className={styles.bubble}>{sent}</p>}
      </div>
      <form
        className={styles.composer}
        onSubmit={(event) => {
          event.preventDefault();
          if (message.trim()) {
            setSent(message);
            setMessage('');
          }
        }}
      >
        <textarea
          aria-label="Ask the assistant"
          rows={2}
          placeholder="Ask about orders… use @ to mention a table"
          value={message}
          onChange={(event) => {
            setMessage(event.target.value);
          }}
        />
        <div>
          <small>⌘I inline in editor</small>
          <IconButton
            aria-label="Send"
            size="sm"
            variant="primary"
            type="submit"
          >
            ↑
          </IconButton>
        </div>
      </form>
    </div>
  );
}
function AssistantSql({
  onQuery,
}: {
  onQuery: (sql: string) => void;
}): React.JSX.Element {
  const [copyState, setCopyState] = useState('Copy SQL');
  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(assistantSql);
      setCopyState('SQL copied');
    } catch {
      setCopyState('Copy unavailable');
    }
  }
  return (
    <div className={styles.sqlCard}>
      <SqlCode sql={assistantSql} />
      <div className={styles.sqlActions}>
        <Button
          size="sm"
          variant="primary"
          onClick={() => {
            onQuery(assistantSql);
          }}
        >
          Run read-only
        </Button>
        <Button
          size="sm"
          onClick={() => {
            onQuery(assistantSql);
          }}
        >
          Open in tab
        </Button>
        <IconButton
          aria-label={copyState}
          size="sm"
          onClick={() => {
            void copy();
          }}
        >
          {copyState === 'SQL copied' ? '✓' : '⧉'}
        </IconButton>
      </div>
    </div>
  );
}
