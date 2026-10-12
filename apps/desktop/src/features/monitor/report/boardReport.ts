// SPDX-License-Identifier: Apache-2.0
// Board sections of a report: one section per widget from its latest run.
import type { BoardDto, BoardRunReply, Widget } from '@openrows/ipc';
import { chartData } from '../boards/boardData';
import type { WidgetRun } from '../boards/useBoardRuns';
import { roundValue } from './model';
import type { Evidence, ReportSection, SummarySection } from './model';

type Runs = Readonly<Record<string, WidgetRun>>;

function runTime(run: WidgetRun | undefined): number | null {
  return run?.status === 'done' ? run.atMs : null;
}

function evidence(
  board: BoardDto,
  widget: Widget,
  run: WidgetRun | undefined,
): Evidence {
  const at = runTime(run);
  const gaps: string[] = [];
  if (!run) gaps.push('Not run: no result to report');
  else if (run.status === 'running')
    gaps.push('Still running: no result to report');
  else if (run.status === 'error') gaps.push(`Run failed: ${run.message}`);
  else if (run.reply.result.truncated)
    gaps.push(`Result truncated by the ${run.reply.result.truncated} limit`);
  const mode =
    run?.status === 'done' && run.reply.readOnly !== 'enforced'
      ? 'classified read'
      : 'read-only';
  return {
    source: `Board “${board.board.name}” · widget “${widget.title}” (${mode} query)`,
    fromMs: at,
    toMs: at,
    scope: 'Database',
    gaps,
  };
}

function resultSection(
  widget: Widget,
  reply: BoardRunReply,
  base: Evidence,
): ReportSection {
  const { result } = reply;
  const id = `widget:${widget.id}`;
  if (widget.kind === 'stat') {
    const cell = result.rows[0]?.[0];
    const gaps = cell === undefined ? [...base.gaps, 'No rows'] : base.gaps;
    return {
      id,
      kind: 'stat',
      title: widget.title,
      evidence: { ...base, gaps },
      value: cell === undefined ? null : (cell ?? 'NULL'),
    };
  }
  const chart =
    widget.kind === 'chart' && widget.chart
      ? chartData(result, widget.chart)
      : null;
  if (chart?.kind === 'chart' && widget.chart)
    return {
      id,
      kind: 'chart',
      title: widget.title,
      evidence: base,
      unit: '',
      mark: widget.chart.mark,
      xKind: chart.xKind,
      cadenceMs: null,
      series: chart.series.map((series) => ({
        name: series.name,
        points: series.points.map((p) => ({
          ...p,
          y: p.y === null ? null : roundValue(p.y),
          state: p.y === null ? 'gap' : 'ok',
        })),
      })),
    };
  const gaps =
    chart?.kind === 'unsuitable' ? [...base.gaps, chart.reason] : base.gaps;
  return {
    id,
    kind: 'table',
    title: widget.title,
    evidence: { ...base, gaps },
    columns: result.columns.map((column) => ({ name: column.name, unit: '' })),
    rows: result.rows.map((row) => row.map((value) => value ?? 'NULL')),
  };
}

function widgetSection(
  board: BoardDto,
  widget: Widget,
  run: WidgetRun | undefined,
): ReportSection {
  const base = evidence(board, widget, run);
  if (run?.status === 'done') return resultSection(widget, run.reply, base);
  return {
    id: `widget:${widget.id}`,
    kind: 'stat',
    title: widget.title,
    evidence: base,
    value: null,
  };
}

function summary(board: BoardDto, runs: Runs): SummarySection {
  const widgets = board.board.widgets;
  const done = widgets.filter((w) => runs[w.id]?.status === 'done').length;
  return {
    id: 'summary',
    kind: 'summary',
    title: 'Summary',
    evidence: {
      source: `Board “${board.board.name}”`,
      ...boardRange(runs),
      scope: 'Database',
      gaps:
        done < widgets.length
          ? [
              `${String(widgets.length - done)} of ${String(widgets.length)} widgets have no result`,
            ]
          : [],
    },
    items: [
      { label: 'Board', value: board.board.name },
      {
        label: 'Widgets with results',
        value: `${String(done)} of ${String(widgets.length)}`,
      },
    ],
  };
}

/** Summary plus the chosen widgets (`include` holds `summary` and widget ids). */
export function boardSections(
  board: BoardDto,
  runs: Runs,
  include: ReadonlySet<string>,
): ReportSection[] {
  return [
    ...(include.has('summary') ? [summary(board, runs)] : []),
    ...board.board.widgets
      .filter((widget) => include.has(widget.id))
      .map((widget) => widgetSection(board, widget, runs[widget.id])),
  ];
}

/** Earliest and latest widget run time. */
export function boardRange(runs: Runs): {
  fromMs: number | null;
  toMs: number | null;
} {
  const times = Object.values(runs).flatMap((run) => {
    const at = runTime(run);
    return at === null ? [] : [at];
  });
  return {
    fromMs: times.length ? Math.min(...times) : null,
    toMs: times.length ? Math.max(...times) : null,
  };
}
