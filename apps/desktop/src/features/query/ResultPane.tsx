// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import type { EngineDto } from '@openrows/ipc';
import type { ExtraResultTab } from '../explain';
import { MessagesPanel } from './MessagesPanel';
import { ResultBar, useMessagesShown } from './ResultBar';
import { t } from './lazyMessages';
import { resultMessages } from './messages';
import { useResultChoice } from './resultChoice';
import type { ResultMeta, RunMessage, RunState } from './run';
import type { ResultView } from './editorSettings';
import type { SelectedCell } from './cell';
import { ResultContent, StackedResults } from './StackedResults';
import styles from './Query.module.css';

type Props = {
  run: RunState;
  /** One result tab per statement, stacked results, or only the last result. */
  view: ResultView;
  /** List server notices and warnings, not only errors. */
  showNotices?: boolean | undefined;
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
  /** A tab beside the result sets (the query plan), shown when it arrives. */
  extra?: ExtraResultTab | null | undefined;
  /** Switch between result tabs and stacked results. */
  onView?: ((view: ResultView) => void) | undefined;
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

/**
 * Whether a run has anything for the pane to show; before the first run the
 * editor takes the whole height and the status line carries the hint.
 */
export function hasResultPane(
  run: RunState,
  extra: ExtraResultTab | null | undefined,
): boolean {
  const waiting =
    run.status === 'idle' ||
    run.status === 'confirm' ||
    run.status === 'parameters';
  return (
    !waiting || run.results.length > 0 || run.messages.length > 0 || !!extra
  );
}

function placeholder(run: RunState): string {
  switch (run.status) {
    case 'starting':
    case 'running':
      return t('running');
    case 'completed':
      return t('statementsReturnedNo');
    default:
      return t('noResult');
  }
}

/** The results in view, whether they stack, and the shown plan or result. */
function shownResult(
  run: RunState,
  view: ResultView,
  extra: ExtraResultTab | null,
  chosen: string | null,
): {
  results: readonly ResultMeta[];
  stacked: boolean;
  shown: ExtraResultTab | null;
  current: ResultMeta | undefined;
} {
  const results = view === 'last' ? run.results.slice(-1) : run.results;
  const stacked = view === 'stacked' && results.length > 1;
  const shown = extra && chosen === extra.id ? extra : null;
  if (shown) return { results, stacked, shown, current: undefined };
  const current = stacked
    ? results.at(-1)
    : (results.find((result) => result.id === chosen) ?? results.at(-1));
  return { results, stacked, shown, current };
}

/** The shown result while it can be exported. */
function exportableOf(
  current: ResultMeta | undefined,
  onExport: Props['onExport'],
): ResultMeta | null {
  return current?.done && onExport ? current : null;
}

/**
 * One result tab per result-producing statement (the newest is shown first),
 * every result stacked, or only the last result.
 */
export function ResultPane({
  run,
  view,
  showNotices,
  engine,
  onCell,
  onExport,
  onExportable,
  extra,
  onView,
}: Props): React.JSX.Element {
  const [chosen, setChosen] = useResultChoice(extra?.id ?? null, run.queryId);
  const { results, stacked, shown, current } = shownResult(
    run,
    view,
    extra ?? null,
    chosen,
  );
  useExportable(onExportable, exportableOf(current, onExport), onExport);
  const [messagesShown, toggleMessages] = useMessagesShown(run.queryId);
  const messages = resultMessages({
    messages: run.messages,
    results,
    current: stacked ? undefined : current,
    showNotices: showNotices ?? true,
  });
  return (
    <section className={styles.results} aria-label={t('results')}>
      <ResultBar
        results={results}
        stacked={stacked}
        current={current}
        extra={extra ?? null}
        extraShown={shown !== null}
        onChoose={setChosen}
        messages={shown ? 0 : messages.length}
        messagesShown={messagesShown}
        onMessages={toggleMessages}
        layout={onView && run.results.length > 1 ? { view, onView } : null}
      />
      {shown ? (
        shown.content
      ) : (
        <ResultBody
          run={run}
          messages={messagesShown ? messages : []}
          engine={engine}
          onCell={onCell}
          onExport={onExport}
          results={results}
          current={current}
          stacked={stacked}
        />
      )}
    </section>
  );
}

/** Messages and the chosen result's grid, every grid stacked, or why there is none. */
function ResultBody({
  run,
  messages,
  results,
  current,
  stacked,
  ...grid
}: Pick<Props, 'run' | 'engine' | 'onCell' | 'onExport'> & {
  messages: readonly RunMessage[];
  results: readonly ResultMeta[];
  current: ResultMeta | undefined;
  stacked: boolean;
}): React.JSX.Element {
  return (
    <>
      <MessagesPanel
        key={current?.id}
        messages={messages}
        statements={run.statements.length}
      />
      {stacked ? (
        <StackedResults
          {...grid}
          statements={run.statements}
          results={results}
        />
      ) : current ? (
        <ResultContent {...grid} statements={run.statements} result={current} />
      ) : (
        <p className={styles.placeholder}>{placeholder(run)}</p>
      )}
    </>
  );
}
