// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import * as ipc from '@openrows/ipc';
import type { ReportFile } from './formats';
import { REPORT_SCHEMA, REPORT_VERSION } from './model';
import type { Report } from './model';
import { ReportDialog } from './ReportDialog';

vi.mock('@openrows/ipc', () => ({ saveReport: vi.fn() }));

const PNG = [137, 80, 78, 71, 13, 10, 26, 10];
const evidence = {
  source: 'Live collector',
  fromMs: 0,
  toMs: 5000,
  scope: 'Server',
  gaps: [],
};
const report: Report = {
  schema: REPORT_SCHEMA,
  version: REPORT_VERSION,
  title: 'Monitor report · shop',
  generatedMs: 5000,
  connection: { name: 'Local', engine: 'postgres', database: 'shop' },
  range: { fromMs: 0, toMs: 5000, label: 'Live · 10 minutes' },
  sections: [
    {
      id: 'metric:Active sessions',
      kind: 'chart',
      title: 'Active sessions',
      evidence,
      unit: 'count',
      mark: 'line',
      xKind: 'time',
      cadenceMs: 5000,
      series: [
        {
          name: 'Active sessions',
          points: [{ x: 0, label: 'a', y: 2, state: 'ok' }],
        },
      ],
    },
    {
      id: 'queries',
      kind: 'table',
      title: 'Top queries',
      evidence,
      columns: [{ name: 'calls', unit: '' }],
      rows: [[4]],
    },
  ],
};
const parts = [
  { value: 'metrics', label: 'Metric charts' },
  { value: 'queries', label: 'Top queries' },
];

beforeEach(() => {
  vi.mocked(ipc.saveReport).mockReset();
  vi.mocked(ipc.saveReport).mockResolvedValue('/tmp/report');
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function open(build = vi.fn(() => Promise.resolve(report))): {
  onClose: () => void;
} {
  const onClose = vi.fn();
  render(
    <ReportDialog
      ranges={[{ value: '0', label: 'Live · 10 minutes' }]}
      parts={parts}
      build={build}
      onClose={onClose}
    />,
  );
  return { onClose };
}

it('offers range, sections and format, then saves the chosen JSON', async () => {
  const build = vi.fn(() => Promise.resolve(report));
  const { onClose } = open(build);
  expect(screen.getByLabelText('Range')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('checkbox', { name: 'Top queries' }));
  expect(build).toHaveBeenLastCalledWith('0', new Set(['metrics']));
  await userEvent.selectOptions(screen.getByLabelText('Format'), 'json');
  await userEvent.click(screen.getByRole('button', { name: 'Export…' }));
  await waitFor(() => {
    expect(onClose).toHaveBeenCalled();
  });
  const [name, file] = vi.mocked(ipc.saveReport).mock.calls[0] ?? [];
  expect(name).toBe('Monitor report · shop');
  expect(file?.format).toBe('json');
  expect(file && 'text' in file ? JSON.parse(file.text) : null).toEqual(report);
});

it('exports one chart as PNG drawn from its SVG', async () => {
  const drawn: string[] = [];
  vi.stubGlobal(
    'Image',
    class {
      onload: (() => void) | null = null;
      set src(url: string) {
        drawn.push(url);
        queueMicrotask(() => this.onload?.());
      }
    },
  );
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:chart');
  vi.spyOn(URL, 'revokeObjectURL').mockReturnValue(undefined);
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    drawImage: vi.fn(),
  } as unknown as CanvasRenderingContext2D);
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation((done) => {
    done(new Blob([new Uint8Array(PNG)], { type: 'image/png' }));
  });
  open();
  await userEvent.selectOptions(await screen.findByLabelText('Format'), 'png');
  expect(await screen.findByLabelText('Chart')).toHaveValue(
    'metric:Active sessions',
  );
  await userEvent.click(screen.getByRole('button', { name: 'Export…' }));
  await waitFor(() => {
    expect(ipc.saveReport).toHaveBeenCalled();
  });
  const file: ReportFile | undefined = vi.mocked(ipc.saveReport).mock
    .calls[0]?.[1];
  expect(file?.format).toBe('png');
  expect(file && 'bytes' in file ? file.bytes.slice(0, 8) : []).toEqual(PNG);
  expect(drawn).toEqual(['blob:chart']);
});

it('exports the chosen table section as CSV', async () => {
  open();
  await userEvent.selectOptions(await screen.findByLabelText('Format'), 'csv');
  expect(await screen.findByLabelText('Table')).toHaveValue(
    'metric:Active sessions',
  );
  await userEvent.selectOptions(screen.getByLabelText('Table'), 'queries');
  await userEvent.click(screen.getByRole('button', { name: 'Export…' }));
  await waitFor(() => {
    expect(ipc.saveReport).toHaveBeenCalledWith(
      'Monitor report · shop Top queries',
      {
        format: 'csv',
        text: 'calls\r\n4\r\n',
      },
    );
  });
});
