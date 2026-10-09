// SPDX-License-Identifier: Apache-2.0
import { lazy, Suspense, useEffect, useState } from 'react';
import { Button } from '@openrows/ui';
import type { EngineDto } from '@openrows/ipc';
import type { ResultMeta, RunMessage, RunState } from './run';
import type { ResultView } from './editorSettings';
import type { RunScope } from './request';
import type { SelectedCell } from './cell';
import styles from './Query.module.css';
const ResultGrid = lazy(() =>
  import('./ResultGrid').then((module) => ({ default: module.ResultGrid })),
);

type Props = {
  run: RunState;
  /** One result tab per statement, or only the last result. */
  view: ResultView;
  /** What ⌘↵ runs, for the empty-state hint. */
  runDefault: RunScope;
  /** Engine of the connection: quoting for copied SQL. */
  engine: EngineDto;
  onCell: (cell: SelectedCell) => void;
  /**
   * Export… for the shown result, or for some of its rows (stored
   * positions); omitted while it cannot be exported.
   */
  onExport?: ((result: ResultMeta, rows?: number[]) => void) | undefined;
  /** Receives Export… of the shown result while it can be exported (Export results command). */
  onExportable?: ((exportShown: (() => void) | null) => void) | undefined;
};

/** Report Export… of `result`, or `null` while there is none. */
function useExportable(
  report: Props['onExportable'],
  result: ResultMeta | null,
  onExport: Props['onExport'],
): void {
  useEffect(() => {
    if (!report) return;
    report(
      result && onExport
        ? () => {
            onExport(result);
          }
        : null,
    );
    return () => {
      report(null);
    };
  }, [report, result, onExport]);
}

function resultLabel(result: ResultMeta, index: number): string {
  return `Result ${String(index + 1)} · statement ${String(result.statement + 1)}`;
}

function placeholder(run: RunState, runDefault: RunScope): string {
  switch (run.status) {
    case 'idle':
    case 'confirm':
    case 'parameters':
      return runDefault === 'all'
        ? 'Run every statement with ⌘↵, or the statement at the cursor with ⇧⌘↵.'
        : 'Run the statement at the cursor with ⌘↵, or every statement with ⇧⌘↵.';
    case 'starting':
    case 'running':
      return 'Running…';
    case 'completed':
      return 'The statements returned no rows.';
    default:
      return 'No result.';
  }
}

/**
 * One result tab per result-producing statement (the newest is shown first),
 * or only the last result.
 */
export function ResultPane({
  run,
  view,
  runDefault,
  engine,
  onCell,
  onExport,
  onExportable,
}: Props): React.JSX.Element {
  const [chosen, setChosen] = useState<string | null>(null);
  const results = view === 'last' ? run.results.slice(-1) : run.results;
  const current =
    results.find((result) => result.id === chosen) ?? results.at(-1);
  const exportable = current?.done && onExport ? current : null;
  useExportable(onExportable, exportable, onExport);
  return (
    <section className={styles.results} aria-label="Results">
      {(results.length > 1 || exportable) && (
        <div className={styles.resultBar}>
          {results.length > 1 && (
            <ResultTabs
              results={results}
              current={current}
              onChoose={setChosen}
            />
          )}
          {exportable && (
            <Button
              size="sm"
              variant="ghost"
              className={styles.exportButton}
              onClick={() => {
                onExport?.(exportable);
              }}
            >
              Export…
            </Button>
          )}
        </div>
      )}
      <Messages messages={run.messages} />
      {!current ? (
        <p className={styles.placeholder}>{placeholder(run, runDefault)}</p>
      ) : current.done || current.rows > 0 ? (
        <Suspense fallback={null}>
          <ResultGrid
            key={current.id}
            result={current}
            engine={engine}
            onCell={onCell}
            onExportRows={
              exportable
                ? (rows) => {
                    onExport?.(exportable, rows);
                  }
                : undefined
            }
          />
        </Suspense>
      ) : (
        <p className={styles.placeholder} role="status">
          Receiving rows… {current.rows.toLocaleString()} so far
        </p>
      )}
    </section>
  );
}

function ResultTabs(props: {
  results: readonly ResultMeta[];
  current: ResultMeta | undefined;
  onChoose: (id: string) => void;
}): React.JSX.Element {
  return (
    <div role="tablist" aria-label="Result sets" className={styles.resultTabs}>
      {props.results.map((result, index) => (
        <button
          key={result.id}
          type="button"
          role="tab"
          aria-selected={result === props.current}
          onClick={() => {
            props.onChoose(result.id);
          }}
        >
          {resultLabel(result, index)}
        </button>
      ))}
    </div>
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
