// SPDX-License-Identifier: Apache-2.0
import { expect, it } from 'vitest';
import type { BoardDto, MonitorReply, SourceStatus } from '@openrows/ipc';
import type { MonitorData } from '../monitorData';
import { boardSections } from './boardReport';
import { csvField, escapeMarkdown } from './escape';
import { reportJson, sectionCsv } from './formats';
import { reportHtml } from './html';
import { reportMarkdown } from './markdown';
import { REPORT_SCHEMA, REPORT_VERSION } from './model';
import type { ChartSection, Report, TableSection } from './model';
import { monitorSections } from './monitorReport';
import { chartSvg } from './svg';

const T0 = Date.UTC(2026, 9, 12, 9, 0, 0);
const HOSTILE = '<script>alert(1)</script>';
const FORMULA = '=HYPERLINK("http://x")\n| **x** <img src=x onerror=alert(1)>';

function status(
  source: SourceStatus['source'],
  state: SourceStatus['state'],
): SourceStatus {
  return {
    source,
    state,
    collectedMs: T0 + 60_000,
    lastSuccessMs: T0 + 60_000,
    intervalMs: 5000,
    durationMs: 1,
    sequence: 1,
    partial: false,
  };
}

const reply: MonitorReply = {
  generation: 'g',
  background: false,
  capabilities: [
    {
      source: 'statements',
      state: { kind: 'ok' },
      scope: 'database',
      intervalMs: 15_000,
      guidance: null,
      coverage: '',
    },
  ],
  sources: [
    status('activity', { kind: 'ok' }),
    status('statements', { kind: 'ok' }),
    status('replication', {
      kind: 'permissionDenied',
      reason: 'needs pg_monitor',
    }),
  ],
};

const data: MonitorData = {
  reply,
  points: [
    {
      metric: 'sessions:active',
      collectedMs: T0 + 5000,
      value: 3,
      state: 'ok',
    },
    {
      metric: 'sessions:active',
      collectedMs: T0 + 10_000,
      value: null,
      state: 'unknown',
    },
    {
      metric: 'sessions:active',
      collectedMs: T0 + 15_000,
      value: 12.5,
      state: 'ok',
    },
  ],
  tables: {
    statements: {
      columns: [
        { name: 'query', unit: '' },
        { name: 'calls', unit: '/s' },
        { name: 'mean_ms', unit: 'ms' },
      ],
      rows: [
        { query: HOSTILE, calls: null, mean_ms: 3.25 },
        { query: FORMULA, calls: 7, mean_ms: -5 },
      ],
      state: 'ok',
      partial: false,
    },
  },
};

function fixture(): Report {
  const range = {
    fromMs: T0,
    toMs: T0 + 60_000,
    label: 'Live · 10 minutes',
    history: false,
  };
  const sections = monitorSections(
    {
      engine: 'postgres',
      data,
      points: data.points,
      range,
      incidents: { reply: null, error: null },
    },
    new Set(['summary', 'metrics', 'queries', 'incidents'] as const),
  );
  return {
    schema: REPORT_SCHEMA,
    version: REPORT_VERSION,
    title: `Report ${HOSTILE}`,
    generatedMs: T0 + 60_000,
    connection: { name: '<b>prod</b>', engine: 'postgres', database: 'shop' },
    range,
    sections,
  };
}

function section<K extends 'chart' | 'table'>(
  report: Report,
  id: string,
  kind: K,
) {
  const found = report.sections.find((s) => s.id === id);
  if (found?.kind !== kind) throw new Error(`no ${kind} ${id}`);
  return found as K extends 'chart' ? ChartSection : TableSection;
}

it('keeps unknown observations and missing intervals as explicit gaps', () => {
  const report = fixture();
  const active = section(report, 'metric:Active sessions', 'chart');
  expect(active.series[0]?.points.map((p) => p.y)).toEqual([3, null, 12.5]);
  expect(active.evidence.gaps).toContain(
    '1 observation: Unknown — awaiting a baseline or server value',
  );
  expect(active.evidence.gaps).toContain(
    'No observation from 2026-10-12T09:00:15.000Z to 2026-10-12T09:01:00.000Z',
  );
  expect(active.evidence).toMatchObject({
    scope: 'Server',
    fromMs: T0,
    toMs: T0 + 60_000,
  });
  expect(section(report, 'metric:Deadlocks/s', 'chart').evidence.gaps).toEqual([
    'No observations in this range',
  ]);
  expect(report.sections[0]?.evidence.gaps).toContain(
    'Source replication: Permission denied',
  );
  expect(sectionCsv(active)).toContain(
    'Active sessions,2026-10-12T09:00:10.000Z,,unknown',
  );
  expect(reportJson(report)).toContain('"y": null');
  const markdown = reportMarkdown(report);
  expect(markdown).toContain(
    '| Active sessions | 12.5 | OK | 3 | 12.5 | 7.75 | 2 | 1 |',
  );
  expect(markdown).toContain(
    '| &lt;script&gt;alert(1)&lt;/script&gt; | unknown | 3.25 |',
  );
  expect(section(report, 'queries', 'table').rows[0]).toEqual([
    HOSTILE,
    null,
    3.25,
  ]);
});

it('prints the same fixture numbers in JSON, CSV, HTML and Markdown', () => {
  const report = fixture();
  const parsed = JSON.parse(reportJson(report)) as Report;
  expect(parsed.schema).toBe('openrows.monitor-report');
  expect(parsed.version).toBe(1);
  const queries = section(parsed, 'queries', 'table');
  const active = section(parsed, 'metric:Active sessions', 'chart');
  expect(queries.rows.map((row) => row[2])).toEqual([3.25, -5]);
  expect(active.series[0]?.points.at(-1)?.y).toBe(12.5);
  const csv = sectionCsv(section(report, 'queries', 'table'));
  expect(csv.split('\r\n')[1]).toBe(`${HOSTILE},,3.25`);
  expect(csv).toContain(',7,-5');
  expect(
    sectionCsv(section(report, 'metric:Active sessions', 'chart')),
  ).toContain(',12.5,ok');
  const html = reportHtml(report);
  const markdown = reportMarkdown(report);
  for (const value of ['3.25', '-5', '12.5', '7']) {
    expect(html).toContain(`<td>${value}</td>`);
    expect(markdown).toContain(`| ${value} |`);
  }
});

it('escapes hostile database text in HTML: no scripts, handlers or external resources', () => {
  const html = reportHtml(fixture());
  const doc = new DOMParser().parseFromString(html, 'text/html');
  expect(
    doc.querySelectorAll('script, img, iframe, object, embed, link, base, form')
      .length,
  ).toBe(0);
  for (const element of doc.querySelectorAll('*'))
    for (const attribute of element.attributes) {
      expect(attribute.name).not.toMatch(/^on/i);
      expect(['src', 'href', 'xlink:href', 'action']).not.toContain(
        attribute.name,
      );
    }
  expect(html).not.toMatch(/<script|url\(|@import/i);
  expect(html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;');
  expect(html).toContain('Content-Security-Policy');
  const cells = [...doc.querySelectorAll('td')].map((td) => td.textContent);
  expect(cells).toContain(HOSTILE);
});

it('escapes Markdown and guards CSV formulas', () => {
  expect(escapeMarkdown(FORMULA)).toBe(
    '=HYPERLINK("http://x") \\| \\*\\*x\\*\\* &lt;img src=x onerror=alert(1)&gt;',
  );
  const markdown = reportMarkdown(fixture());
  expect(markdown).not.toMatch(/<script|<img|<b>/);
  const row = markdown.split('\n').find((line) => line.includes('HYPERLINK'));
  expect(row?.match(/(?<!\\)\|/g)).toHaveLength(4);
  expect(csvField('=1+1')).toBe("'=1+1");
  expect(csvField('@SUM(A1)')).toBe("'@SUM(A1)");
  expect(csvField('+cmd|x')).toBe("'+cmd|x");
  expect(csvField('-2')).toBe('-2');
  expect(csvField('a,"b"')).toBe('"a,""b"""');
  expect(csvField(null)).toBe('');
  expect(sectionCsv(section(fixture(), 'queries', 'table'))).toContain(
    `"'=HYPERLINK(""http://x"")\n| **x** <img src=x onerror=alert(1)>",7,-5`,
  );
});

it('draws a standalone SVG with gaps as breaks and escaped labels', () => {
  const chart = section(fixture(), 'metric:Active sessions', 'chart');
  const svg = chartSvg({ ...chart, title: HOSTILE });
  const doc = new DOMParser().parseFromString(svg, 'image/svg+xml');
  expect(doc.documentElement.nodeName).toBe('svg');
  expect(doc.querySelector('parsererror')).toBeNull();
  expect(doc.querySelectorAll('script').length).toBe(0);
  expect(
    doc.querySelector('path')?.getAttribute('d')?.match(/M/g),
  ).toHaveLength(2);
  expect(svg).toContain('&lt;script&gt;');
  expect(doc.querySelectorAll('line')).toHaveLength(1);
});

it('reports board widgets with run evidence, NULLs and missing runs', () => {
  const board: BoardDto = {
    id: 'b',
    reviewed: true,
    updatedAtMs: 1,
    board: {
      version: 1,
      name: 'Sales',
      params: [],
      refreshSecs: null,
      widgets: [
        {
          id: 'w1',
          title: 'Orders',
          kind: 'table',
          sql: 'SELECT 1',
          chart: null,
          limits: { maxRows: 1000, timeoutMs: 15_000, maxBytes: 1_048_576 },
        },
        {
          id: 'w2',
          title: 'Revenue',
          kind: 'stat',
          sql: 'SELECT 1',
          chart: null,
          limits: { maxRows: 1000, timeoutMs: 15_000, maxBytes: 1_048_576 },
        },
      ],
    },
  };
  const reply = {
    result: {
      columns: [{ name: 'name', kind: 'text' as const }],
      rows: [[HOSTILE], [null]],
      truncated: 'rows' as const,
    },
    readOnly: 'enforced' as const,
    elapsedMs: 3,
  };
  const sections = boardSections(
    board,
    { w1: { status: 'done', reply, atMs: T0 } },
    new Set(['summary', 'w1', 'w2']),
  );
  expect(sections.map((s) => s.kind)).toEqual(['summary', 'table', 'stat']);
  expect(sections[0]?.evidence.gaps).toEqual(['1 of 2 widgets have no result']);
  expect(sections[1]).toMatchObject({
    rows: [[HOSTILE], ['NULL']],
    evidence: {
      fromMs: T0,
      scope: 'Database',
      gaps: ['Result truncated by the rows limit'],
    },
  });
  expect(sections[2]).toMatchObject({
    value: null,
    evidence: { gaps: ['Not run: no result to report'] },
  });
});
