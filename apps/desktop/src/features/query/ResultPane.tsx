// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import { Button, Icon } from '@openrows/ui';
import type { EngineDto } from '@openrows/ipc';
import type { ExtraResultTab } from '../explain';
import { MessagesPanel } from './MessagesPanel';
import { resultMessages } from './messages';
import { useResultChoice } from './resultChoice';
import type { ResultMeta, RunState } from './run';
import type { ResultView } from './editorSettings';
import type { RunScope } from './request';
import type { SelectedCell } from './cell';
import { ResultContent, resultLabel, StackedResults } from './StackedResults';
import styles from './Query.module.css';

type Props = {
  run: RunState;
  /** One result tab per statement, stacked results, or only the last result. */
  view: ResultView;
  /** What ⌘↵ runs, for the empty-state hint. */
  runDefault: RunScope;
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
 * every result stacked, or only the last result.
 */
export function ResultPane({
  run,
  view,
  runDefault,
  showNotices,
  engine,
  onCell,
  onExport,
  onExportable,
  extra,
}: Props): React.JSX.Element {
  const [chosen, setChosen] = useResultChoice(extra?.id ?? null, run.queryId);
  const results = view === 'last' ? run.results.slice(-1) : run.results;
  const stacked = view === 'stacked' && results.length > 1;
  const shown = extra && chosen === extra.id ? extra : null;
  const current = shown
    ? undefined
    : stacked
      ? results.at(-1)
      : (results.find((result) => result.id === chosen) ?? results.at(-1));
  const exportable = current?.done && onExport ? current : null;
  useExportable(onExportable, exportable, onExport);
  return (
    <section className={styles.results} aria-label="Results">
      <ResultBar
        results={results}
        stacked={stacked}
        current={current}
        extra={extra ?? null}
        extraShown={shown !== null}
        onChoose={setChosen}
        exportable={exportable}
        onExport={onExport}
      />
      {shown ? (
        shown.content
      ) : (
        <ResultBody
          run={run}
          runDefault={runDefault}
          showNotices={showNotices}
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

/** Result tabs (with the extra tab) and Export…, when there is any. */
function ResultBar(
  props: TabsProps & {
    exportable: ResultMeta | null;
    onExport: Props['onExport'];
  },
): React.JSX.Element | null {
  const { exportable, extra } = props;
  const tabs = (!props.stacked && props.results.length > 1) || extra !== null;
  if (!tabs && !exportable) return null;
  return (
    <div className={styles.resultBar}>
      {tabs && <ResultTabs {...props} />}
      {exportable && (
        <Button
          size="sm"
          variant="ghost"
          className={styles.exportButton}
          onClick={() => {
            props.onExport?.(exportable);
          }}
        >
          Export…
        </Button>
      )}
    </div>
  );
}

/** Messages and the chosen result's grid, every grid stacked, or why there is none. */
function ResultBody({
  run,
  runDefault,
  showNotices,
  results,
  current,
  stacked,
  ...grid
}: Pick<
  Props,
  'run' | 'runDefault' | 'showNotices' | 'engine' | 'onCell' | 'onExport'
> & {
  results: readonly ResultMeta[];
  current: ResultMeta | undefined;
  stacked: boolean;
}): React.JSX.Element {
  return (
    <>
      <ResultMessages
        run={run}
        results={results}
        current={stacked ? undefined : current}
        showNotices={showNotices}
      />
      {stacked ? (
        <StackedResults {...grid} results={results} />
      ) : current ? (
        <ResultContent {...grid} result={current} />
      ) : (
        <p className={styles.placeholder}>{placeholder(run, runDefault)}</p>
      )}
    </>
  );
}

type TabsProps = {
  results: readonly ResultMeta[];
  current: ResultMeta | undefined;
  extra: ExtraResultTab | null;
  extraShown: boolean;
  stacked: boolean;
  onChoose: (id: string | null) => void;
};

function ResultTabs(props: TabsProps): React.JSX.Element {
  const { extra } = props;
  return (
    <div role="tablist" aria-label="Result sets" className={styles.resultTabs}>
      {props.stacked ? (
        <button
          type="button"
          role="tab"
          aria-selected={!props.extraShown}
          onClick={() => {
            props.onChoose(null);
          }}
        >
          Results
        </button>
      ) : (
        props.results.map((result, index) => (
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
        ))
      )}
      {extra && (
        <span className={styles.extraTab}>
          <button
            type="button"
            role="tab"
            aria-selected={props.extraShown}
            onClick={() => {
              props.onChoose(extra.id);
            }}
          >
            {extra.label}
          </button>
          <button
            type="button"
            className={styles.extraClose}
            aria-label={`Close ${extra.label}`}
            title={`Close ${extra.label}`}
            onClick={extra.onClose}
          >
            <Icon name="close" />
          </button>
        </span>
      )}
    </div>
  );
}

/** Messages of the shown result; notices unless turned off. */
function ResultMessages(props: {
  run: RunState;
  results: readonly ResultMeta[];
  current: ResultMeta | undefined;
  showNotices: boolean | undefined;
}): React.JSX.Element {
  const { run, results, current } = props;
  return (
    <MessagesPanel
      key={current?.id}
      messages={resultMessages({
        messages: run.messages,
        results,
        current,
        showNotices: props.showNotices ?? true,
      })}
      statements={run.statements.length}
    />
  );
}
