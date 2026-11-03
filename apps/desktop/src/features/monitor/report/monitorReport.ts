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
import { t } from '../messages/lazyReport';

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
  if (!scope) return t('unknown');
  return scope === 'server' ? t('server') : t('database');
}

function summarySection(input: MonitorReportInput): SummarySection {
  const reply = input.data?.reply;
  const items = (reply?.sources ?? []).map((status) => {
    const scope = reply?.capabilities.find(
      (c) => c.source === status.source,
    )?.scope;
    const kind = stateLabel(status.state.kind);
    const state =
      'reason' in status.state
        ? t('stateWithReason', { state: kind, reason: status.state.reason })
        : kind;
    return {
      label: t('sourceSource', { source: status.source }),
      value: t('sourceSummary', {
        state,
        scope: scopeName(scope),
        time: timeText(status.lastSuccessMs),
      }),
    };
  });
  const gaps = (reply?.sources ?? []).flatMap((status) =>
    status.state.kind === 'ok'
      ? []
      : [
          t('sourceSourceKind', {
            source: status.source,
            kind: stateLabel(status.state.kind),
          }),
        ],
  );
  return {
    id: 'summary',
    kind: 'summary',
    title: t('summary'),
    evidence: {
      source: t('collectorSourceStatus'),
      fromMs: input.range.fromMs,
      toMs: input.range.toMs,
      scope: 'Connection',
      gaps: reply ? gaps : [t('noCollectorReply')],
    },
    items: [
      { label: t('engine'), value: input.engine },
      {
        label: t('window'),
        value: input.range.history ? t('storedLocalHistory') : t('liveWindow'),
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
        ? t('localHistoryStored')
        : t('liveCollectorMetric', { metric: metricSource(metric) }),
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
  if (!status) gaps.push(t('statementStatisticsNot'));
  else if (status.state.kind !== 'ok')
    gaps.push(
      'reason' in status.state
        ? t('statementsKindReason', {
            kind: stateLabel(status.state.kind),
            reason: status.state.reason,
          })
        : t('statementsKind', { kind: stateLabel(status.state.kind) }),
    );
  if (table?.partial || status?.partial) gaps.push(t('partialCoverageRows'));
  if (table && table.state !== 'ok')
    gaps.push(t('sampleStateState', { state: stateLabel(table.state) }));
  if (total > MAX_QUERIES)
    gaps.push(
      t('showingFirstMax', {
        maxQueries: String(MAX_QUERIES),
        total: String(total),
      }),
    );
  if (input.range.history) gaps.push(t('topQueriesCover'));
  gaps.push(t('unknownResetIntervals'));
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
    title: t('topQueries'),
    evidence: {
      source: t('liveCollectorStatements'),
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
  t('status'),
  t('rule'),
  t('severity'),
  t('scope'),
  t('metric'),
  t('started'),
  t('ended'),
  t('onset'),
  t('peak'),
  t('last'),
];

function incidentRow(incident: Incident): Cell[] {
  return [
    incident.endedMs === null ? t('open') : t('resolved'),
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
  const gaps = [t('incidentsRetainedApp')];
  if (error) gaps.push(t('incidentsUnavailableError', { error }));
  else if (!reply) gaps.push(t('incidentsWereNot'));
  if (incidents.some((i) => i.startedMs === null))
    gaps.push(t('someIncidentStart'));
  return {
    id: 'incidents',
    kind: 'table',
    title: t('incidents'),
    evidence: {
      source: t('deterministicIncidentRules'),
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
