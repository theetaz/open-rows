// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SegmentedControl } from '@openrows/ui';
import type { BoardResult, BoardRunReply, Widget } from '@openrows/ipc';
import { BoardChart } from './BoardChart';
import { chartData, statValue, tableData } from './boardData';
import type { WidgetRun } from './useBoardRuns';
import styles from './Boards.module.css';

const VISIBLE_ROWS = 200;
const VIEWS = [
  { value: 'chart', label: 'Chart' },
  { value: 'table', label: 'Table' },
] as const;

/** The result as an accessible table; the same values the chart draws. */
function ResultTable(props: {
  title: string;
  result: BoardResult;
}): React.JSX.Element {
  const { columns, rows } = tableData(props.result);
  return (
    <div className={styles.tableWrap}>
      <table className={styles.table}>
        <caption className={styles.note}>
          {props.title}:{' '}
          {rows.length > VISIBLE_ROWS
            ? `first ${String(VISIBLE_ROWS)} of `
            : ''}
          {String(rows.length)} rows
        </caption>
        <thead>
          <tr>
            {columns.map((column, index) => (
              <th key={index} scope="col">
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.slice(0, VISIBLE_ROWS).map((row, index) => (
            <tr key={index}>
              {row.map((cell, at) => (
                <td key={at}>{cell}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function summary(reply: BoardRunReply): string {
  const rows = reply.result.rows.length;
  const truncated = reply.result.truncated
    ? ` · stopped at the ${reply.result.truncated === 'rows' ? 'row' : 'byte'} limit`
    : '';
  const mode = reply.readOnly === 'enforced' ? 'read-only' : 'classified read';
  return `${String(rows)} rows · ${String(Math.round(reply.elapsedMs ?? 0))} ms · ${mode}${truncated}`;
}

function Body(props: {
  widget: Widget;
  result: BoardResult;
  view: 'chart' | 'table';
}): React.JSX.Element {
  const { widget, result } = props;
  if (widget.kind === 'stat')
    return <p className={styles.stat}>{statValue(result)}</p>;
  if (widget.kind === 'table' || !widget.chart || props.view === 'table')
    return <ResultTable title={widget.title} result={result} />;
  const data = chartData(result, widget.chart);
  if (data.kind === 'unsuitable')
    return (
      <>
        <p className={styles.note}>{data.reason}</p>
        <ResultTable title={widget.title} result={result} />
      </>
    );
  return (
    <BoardChart
      title={widget.title}
      mark={widget.chart.mark}
      xKind={data.xKind}
      series={data.series}
    />
  );
}

/** One widget: header with result summary, then its chart, stat or table. */
export function WidgetCard(props: {
  widget: Widget;
  run: WidgetRun | undefined;
  onEdit: () => void;
  onRemove: () => void;
}): React.JSX.Element {
  const [view, setView] = useState<'chart' | 'table'>('chart');
  const { widget, run } = props;
  return (
    <article className={styles.widget} aria-label={widget.title}>
      <header>
        <h3 title={widget.title}>{widget.title}</h3>
        {widget.kind === 'chart' && (
          <SegmentedControl
            aria-label={`${widget.title} view`}
            options={VIEWS}
            value={view}
            onValueChange={setView}
            size="compact"
          />
        )}
        <Button size="sm" variant="ghost" onClick={props.onEdit}>
          Edit
        </Button>
        <Button size="sm" variant="ghost" onClick={props.onRemove}>
          Remove
        </Button>
      </header>
      <div>
        {!run && <p className={styles.note}>Not run yet.</p>}
        {run?.status === 'running' && <p className={styles.note}>Running…</p>}
        {run?.status === 'error' && (
          <p role="alert" className={styles.error}>
            {run.message}
          </p>
        )}
        {run?.status === 'done' && (
          <>
            <p className={styles.note}>{summary(run.reply)}</p>
            <Body widget={widget} result={run.reply.result} view={view} />
          </>
        )}
      </div>
    </article>
  );
}
