// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { Button, SegmentedControl } from '@openrows/ui';
import type { BoardResult, BoardRunReply, Widget } from '@openrows/ipc';
import { BoardChart } from './BoardChart';
import { chartData, statValue, tableData } from './boardData';
import type { WidgetRun } from './useBoardRuns';
import styles from './Boards.module.css';
import { t } from '../messages/lazyBoards';

const VISIBLE_ROWS = 200;
const VIEWS = [
  { value: 'chart', label: t('chart') },
  { value: 'table', label: t('table') },
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
          {rows.length > VISIBLE_ROWS
            ? t('titleFirstVisible', {
                title: props.title,
                visibleRows: String(VISIBLE_ROWS),
                count: String(rows.length),
              })
            : t('titleCountRows', {
                title: props.title,
                count: String(rows.length),
              })}
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
  const parts = [
    t('rowsShown', { rows: String(rows) }),
    `${String(Math.round(reply.elapsedMs ?? 0))} ms`,
    reply.readOnly === 'enforced' ? t('readOnlyMode') : t('classifiedRead'),
  ];
  if (reply.result.truncated)
    parts.push(
      reply.result.truncated === 'rows'
        ? t('stoppedRowLimit')
        : t('stoppedByteLimit'),
    );
  return parts.join(' · ');
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
            aria-label={t('titleView', { title: widget.title })}
            options={VIEWS}
            value={view}
            onValueChange={setView}
            size="compact"
          />
        )}
        <Button size="sm" variant="ghost" onClick={props.onEdit}>
          {t('edit')}
        </Button>
        <Button size="sm" variant="ghost" onClick={props.onRemove}>
          {t('remove')}
        </Button>
      </header>
      <div>
        {!run && <p className={styles.note}>{t('notRunYet')}</p>}
        {run?.status === 'running' && (
          <p className={styles.note}>{t('running')}</p>
        )}
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
