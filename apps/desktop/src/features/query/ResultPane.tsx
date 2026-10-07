// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useState } from 'react';
import type { ResultMeta, RunMessage, RunState } from './run';
import type { SelectedCell } from './cell';
import styles from './Query.module.css';
const ResultGrid = lazy(() =>
  import('./ResultGrid').then((module) => ({ default: module.ResultGrid })),
);

type Props = {
  run: RunState;
  onCell: (cell: SelectedCell) => void;
};

function resultLabel(result: ResultMeta, index: number): string {
  return `Result ${String(index + 1)} · statement ${String(result.statement + 1)}`;
}

function placeholder(run: RunState): string {
  switch (run.status) {
    case 'idle':
    case 'confirm':
      return 'Run the statement at the cursor with ⌘↵, or every statement with ⇧⌘↵.';
    case 'starting':
    case 'running':
      return 'Running…';
    case 'completed':
      return 'The statements returned no rows.';
    default:
      return 'No result.';
  }
}

/** One result tab per result-producing statement; the newest is shown first. */
export function ResultPane({ run, onCell }: Props): React.JSX.Element {
  const [chosen, setChosen] = useState<string | null>(null);
  const results = run.results;
  const current =
    results.find((result) => result.id === chosen) ?? results.at(-1);
  return (
    <section className={styles.results} aria-label="Results">
      {results.length > 1 && (
        <div
          role="tablist"
          aria-label="Result sets"
          className={styles.resultTabs}
        >
          {results.map((result, index) => (
            <button
              key={result.id}
              type="button"
              role="tab"
              aria-selected={result === current}
              onClick={() => {
                setChosen(result.id);
              }}
            >
              {resultLabel(result, index)}
            </button>
          ))}
        </div>
      )}
      <Messages messages={run.messages} />
      {!current ? (
        <p className={styles.placeholder}>{placeholder(run)}</p>
      ) : current.done ? (
        <Suspense fallback={null}>
          <ResultGrid key={current.id} result={current} onCell={onCell} />
        </Suspense>
      ) : (
        <p className={styles.placeholder} role="status">
          Receiving rows… {current.rows.toLocaleString()} so far
        </p>
      )}
    </section>
  );
}

function Messages({
  messages,
}: {
  messages: readonly RunMessage[];
}): React.JSX.Element | null {
  if (!messages.length) return null;
  return (
    <ul className={styles.messages} aria-label="Messages">
      {messages.map((message, index) => (
        <li key={index} data-level={message.level}>
          {message.level === 'error' ? 'ERROR' : message.level.toUpperCase()}
          {message.code ? ` ${message.code}` : ''}: {message.text}
        </li>
      ))}
    </ul>
  );
}
