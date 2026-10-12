// SPDX-License-Identifier: Apache-2.0
// Monitor sections of a report: source summary, overview metric charts, top
// queries and incidents, each with its evidence and explicit gaps.
import type { EngineDto, Incident, IncidentsReply, Scope } from '@openrows/ipc';
import { metricPoints } from '../metrics';
import type { Metric } from '../metrics';
import type { MonitorData, Point } from '../monitorData';
import {
  metricCadence,
  metricSource,
  overviewMetrics,
} from '../overviewMetrics';
import { stateLabel } from '../SourceState';
import { metricGaps } from './gaps';
import { roundValue, timeText } from './model';
import type {
  Cell,
  ChartSection,
  ReportSection,
  SummarySection,
  TableSection,
} from './model';

export type MonitorPart = 'summary' | 'metrics' | 'queries' | 'incidents';
export type ReportRange = {
  fromMs: number;
  toMs: number;
  label: string;
  history: boolean;
};
type MonitorReportInput = {
  engine: EngineDto;
  data: MonitorData | null;
  /** Scalar observations of `range`: live window or stored history. */
  points: readonly Point[];
  range: ReportRange;
  incidents: { reply: IncidentsReply | null; error: string | null };
};

/** Most ranked statements a report lists. */
const MAX_QUERIES = 100;
/** Live top-query rankings cover the collector's ten-minute window. */
const LIVE_WINDOW_MS = 600_000;

function scopeName(scope: Scope | undefined): string {
  if (!scope) return 'Unknown';
  return scope === 'server' ? 'Server' : 'Database';
}

function summarySection(input: MonitorReportInput): SummarySection {
  const reply = input.data?.reply;
  const items = (reply?.sources ?? []).map((status) => {
    const scope = reply?.capabilities.find(
      (c) => c.source === status.source,
    )?.scope;
    const reason = 'reason' in status.state ? `: ${status.state.reason}` : '';
    return {
      label: `Source ${status.source}`,
      value: `${stateLabel(status.state.kind)}${reason} · ${scopeName(scope)} scope · last success ${timeText(status.lastSuccessMs)}`,
    };
  });
  const gaps = (reply?.sources ?? []).flatMap((status) =>
    status.state.kind === 'ok'
      ? []
      : [`Source ${status.source}: ${stateLabel(status.state.kind)}`],
  );
  return {
    id: 'summary',
    kind: 'summary',
    title: 'Summary',
    evidence: {
      source: 'Collector source status',
      fromMs: input.range.fromMs,
      toMs: input.range.toMs,
      scope: 'Connection',
      gaps: reply
        ? gaps
        : ['No collector reply: the live monitor has not sampled yet'],
    },
    items: [
      { label: 'Engine', value: input.engine },
      {
        label: 'Window',
        value: input.range.history ? 'Stored local history' : 'Live window',
      },
      ...items,
    ],
  };
}

function metricSection(
  input: MonitorReportInput,
  metric: Metric,
): ChartSection {
  const { range } = input;
  const cadence = metricCadence(
    {
      data: input.data,
      duration: range.toMs - range.fromMs,
      history: range.history,
    },
    metric,
  );
  const points = metricPoints(input.points, metric).map((point) => ({
    x: point.collectedMs,
    label: timeText(point.collectedMs),
    y: point.value === null ? null : roundValue(point.value),
    state: point.state,
  }));
  return {
    id: `metric:${metric.title}`,
    kind: 'chart',
    title: metric.title,
    evidence: {
      source: range.history
        ? 'Local history (stored aggregates)'
        : `Live collector · ${metricSource(metric)} source`,
      fromMs: range.fromMs,
      toMs: range.toMs,
      scope: metric.scope,
      gaps: metricGaps(points, range, cadence),
    },
    unit: metric.unit,
    mark: 'line',
    xKind: 'time',
    cadenceMs: cadence,
    series: [{ name: metric.title, points }],
  };
}

function cell(value: string | number | null | undefined): Cell {
  if (value === undefined || value === null) return null;
  return typeof value === 'number' ? roundValue(value) : value;
}

function queryGaps(input: MonitorReportInput, total: number): string[] {
  const status = input.data?.reply.sources.find(
    (s) => s.source === 'statements',
  );
  const table = input.data?.tables.statements;
  const gaps: string[] = [];
  if (!status)
    gaps.push(
      'Statement statistics are not collected for this engine or not sampled yet',
    );
  else if (status.state.kind !== 'ok')
    gaps.push(
      `Statements: ${stateLabel(status.state.kind)}${'reason' in status.state ? `: ${status.state.reason}` : ''}`,
    );
  if (table?.partial || status?.partial)
    gaps.push('Partial coverage: rows were truncated or evicted');
  if (table && table.state !== 'ok')
    gaps.push(`Sample state: ${stateLabel(table.state)}`);
  if (total > MAX_QUERIES)
    gaps.push(
      `Showing the first ${String(MAX_QUERIES)} of ${String(total)} ranked statements`,
    );
  if (input.range.history)
    gaps.push(
      'Top queries cover the live ten-minute window, not the selected history range',
    );
  gaps.push('Unknown or reset intervals are empty cells, never zeros');
  return gaps;
}

function queriesSection(input: MonitorReportInput): TableSection {
  const table = input.data?.tables.statements;
  const status = input.data?.reply.sources.find(
    (s) => s.source === 'statements',
  );
  const scope = input.data?.reply.capabilities.find(
    (c) => c.source === 'statements',
  )?.scope;
  const toMs = status?.collectedMs ?? null;
  const rows = table?.rows ?? [];
  return {
    id: 'queries',
    kind: 'table',
    title: 'Top queries',
    evidence: {
      source: 'Live collector · statements source (summed live-window deltas)',
      fromMs: toMs === null ? null : toMs - LIVE_WINDOW_MS,
      toMs,
      scope: scopeName(scope),
      gaps: queryGaps(input, rows.length),
    },
    columns: table?.columns ?? [],
    rows: rows
      .slice(0, MAX_QUERIES)
      .map((row) =>
        (table?.columns ?? []).map((column) => cell(row[column.name])),
      ),
  };
}

function overlaps(incident: Incident, fromMs: number, toMs: number): boolean {
  return (
    (incident.endedMs ?? Infinity) >= fromMs &&
    (incident.startedMs ?? -Infinity) <= toMs
  );
}

const INCIDENT_COLUMNS = [
  'Status',
  'Rule',
  'Severity',
  'Scope',
  'Metric',
  'Started',
  'Ended',
  'Onset',
  'Peak',
  'Last',
];

function incidentRow(incident: Incident): Cell[] {
  return [
    incident.endedMs === null ? 'Open' : 'Resolved',
    incident.rule.title,
    incident.rule.severity,
    scopeName(incident.scope),
    incident.rule.metric,
    timeText(incident.startedMs),
    incident.endedMs === null ? 'open' : timeText(incident.endedMs),
    cell(incident.onset[0]?.value),
    cell(incident.peak.value),
    cell(incident.last.value),
  ];
}

function incidentsSection(input: MonitorReportInput): TableSection {
  const { reply, error } = input.incidents;
  const { fromMs, toMs } = input.range;
  const incidents = (reply?.incidents ?? []).filter((i) =>
    overlaps(i, fromMs, toMs),
  );
  const gaps = [
    'Incidents are retained for this app session only; earlier incidents are unknown',
  ];
  if (error) gaps.push(`Incidents unavailable: ${error}`);
  else if (!reply) gaps.push('Incidents were not loaded');
  if (incidents.some((i) => i.startedMs === null))
    gaps.push('Some incident start times are unknown');
  return {
    id: 'incidents',
    kind: 'table',
    title: 'Incidents',
    evidence: {
      source: 'Deterministic incident rules on live samples',
      fromMs,
      toMs,
      scope: 'Server and database',
      gaps,
    },
    columns: INCIDENT_COLUMNS.map((name) => ({ name, unit: '' })),
    rows: incidents.map(incidentRow),
  };
}

/** Sections of the chosen parts, in fixed order. */
export function monitorSections(
  input: MonitorReportInput,
  parts: ReadonlySet<MonitorPart>,
): ReportSection[] {
  return [
    ...(parts.has('summary') ? [summarySection(input)] : []),
    ...(parts.has('metrics')
      ? overviewMetrics(input.engine).map((metric) =>
          metricSection(input, metric),
        )
      : []),
    ...(parts.has('queries') ? [queriesSection(input)] : []),
    ...(parts.has('incidents') ? [incidentsSection(input)] : []),
  ];
}
