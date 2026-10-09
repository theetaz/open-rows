// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { filterLevel } from './messages';
import type { LevelFilter } from './messages';
import type { RunMessage } from './run';
import styles from './MessagesPanel.module.css';

const LEVELS: readonly { value: LevelFilter; label: string }[] = [
  { value: 'all', label: 'All' },
  { value: 'error', label: 'Errors' },
  { value: 'warning', label: 'Warnings' },
  { value: 'info', label: 'Info' },
];

const count = (messages: readonly RunMessage[], level: LevelFilter): number =>
  filterLevel(messages, level).length;

function line(message: RunMessage): string {
  const level =
    message.level === 'error' ? 'ERROR' : message.level.toUpperCase();
  return `${level}${message.code ? ` ${message.code}` : ''}: ${message.text}`;
}

/**
 * Errors and server notices of the shown result, filtered by level; each
 * line names its statement when the run had several.
 */
export function MessagesPanel(props: {
  messages: readonly RunMessage[];
  /** Statements in the run; one hides the statement labels. */
  statements: number;
}): React.JSX.Element | null {
  const [level, setLevel] = useState<LevelFilter>('all');
  const { messages } = props;
  if (!messages.length) return null;
  const shown = filterLevel(messages, level);
  return (
    <section className={styles.panel} aria-label="Messages panel">
      <div className={styles.bar} role="toolbar" aria-label="Message level">
        <strong>Messages</strong>
        {LEVELS.map((item) => (
          <button
            key={item.value}
            type="button"
            aria-pressed={level === item.value}
            onClick={() => {
              setLevel(item.value);
            }}
          >
            {item.label} {count(messages, item.value)}
          </button>
        ))}
      </div>
      <ul className={styles.list} aria-label="Messages">
        {shown.map((message, index) => (
          <li key={index} data-level={message.level}>
            {props.statements > 1 && <small>#{message.statement + 1}</small>}
            {line(message)}
          </li>
        ))}
        {!shown.length && (
          <li className={styles.empty}>No messages at this level.</li>
        )}
      </ul>
    </section>
  );
}
