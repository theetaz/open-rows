// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import { parseReport, validateReport } from './validate';
import type { Report } from './model';
const report: Report = {
  schema: 'openrows.monitor-report',
  version: 1,
  title: 'Evidence',
  generatedMs: 1000,
  connection: { name: 'fixture', database: 'fixture', engine: 'PostgreSQL' },
  range: { fromMs: 1000, toMs: 1000, label: 'One sample' },
  sections: [
    {
      kind: 'chart',
      id: 'sample',
      title: 'Sample',
      evidence: {
        source: 'fixture',
        scope: 'Database',
        fromMs: 1000,
        toMs: 1000,
        gaps: ['Unknown intervals remain gaps'],
      },
      unit: 'count',
      mark: 'scatter',
      xKind: 'time',
      cadenceMs: null,
      series: [
        {
          name: 'active',
          points: [{ x: 1000, label: '1000', y: null, state: 'gap' }],
        },
      ],
    },
  ],
};
describe('shared report validation', () => {
  it('accepts the shared document without replacing gaps with zeros', () => {
    expect(parseReport(JSON.stringify(report))).toEqual(report);
  });
  it('refuses invalid chart marks, nonfinite values, arbitrary sections and oversized documents', () => {
    const invalid = structuredClone(report);
    invalid.generatedMs = Infinity;
    expect(() => validateReport(invalid)).toThrow();
    const json = JSON.stringify(report).replace('scatter', 'script');
    expect(() => parseReport(json)).toThrow();
    expect(() =>
      parseReport(
        JSON.stringify({
          ...report,
          sections: [{ kind: 'html', html: '<script>bad()</script>' }],
        }),
      ),
    ).toThrow();
    expect(() => parseReport(' '.repeat(65537))).toThrow();
  });
});
