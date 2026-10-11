// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import type { SampleState, Source } from '@openrows/ipc';
import { SourcePanel } from './SourcePanel';
import { decodeTable } from './monitorData';
import { monitorBytes, monitorProfile, monitorReply } from './monitorFixture';
import { MetricChart, chartPath } from './MetricChart';
import { TopQueries } from './TopQueries';
import { explainQuery } from '@openrows/ipc';
import { BlockingTree } from './BlockingTree';
import { MonitorOverview } from './MonitorOverview';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
const sources: Source[] = [
  'activity',
  'database',
  'tables',
  'indexes',
  'statements',
  'replication',
  'io',
];
const states: SampleState[] = [
  { kind: 'unknown' },
  { kind: 'gap' },
  { kind: 'counterReset' },
  {
    kind: 'permissionDenied',
    reason: 'Ask for pg_monitor / pg_read_all_stats',
  },
  { kind: 'unavailable', reason: 'Not supported by this source' },
];
it('bounds a wide blocking tree instead of mounting an overflow marker per waiter', () => {
  const table = decodeTable(monitorBytes('activity'));
  table.rows = Array.from({ length: 1000 }, (_, index) => ({
    pid: String(index + 1),
    blockers: '0',
    state: 'active',
  }));
  render(<BlockingTree table={table} />);
  expect(screen.getAllByRole('listitem')).toHaveLength(100);
  expect(
    screen.getByText('Graph display limit (100 nodes)'),
  ).toBeInTheDocument();
});
it('connects valid history buckets from the current-plus-previous range without inventing gaps', () => {
  const points = [0, 24_001, 48_002].map((collectedMs) => ({
    metric: 'sessions:active',
    collectedMs,
    value: 2,
    state: 'ok',
  }));
  render(
    <MonitorOverview
      data={null}
      points={points}
      previous={[]}
      now={60_000}
      duration={3_600_000}
      end={60_000}
      history
    />,
  );
  const chart = screen.getByRole('img', { name: /Active sessions trend/ });
  expect(chart.querySelector('path')?.getAttribute('d')).toContain('L');
});
it.each(sources)(
  'preserves every missing/capability state in %s without rendering stale source rows',
  (source) => {
    for (const state of states) {
      const reply = monitorReply();
      const status = reply.sources.find((item) => item.source === source);
      if (!status) throw new Error('fixture status');
      status.state = state;
      const mounted = render(
        <SourcePanel
          panel={source}
          profile={monitorProfile}
          database="orders"
          sessionId="s"
          onOpenSql={vi.fn()}
          monitor={{
            data: {
              reply,
              tables: { [source]: decodeTable(monitorBytes(source)) },
              points: [],
            },
            now: 1000,
            paused: false,
            setPaused: vi.fn(),
            interval: 5000,
            setInterval: vi.fn(),
            error: null,
          }}
        />,
      );
      expect(screen.queryByRole('grid')).not.toBeInTheDocument();
      if ('reason' in state)
        expect(screen.getByRole('status')).toHaveTextContent(state.reason);
      mounted.unmount();
    }
  },
);
it('keeps chart gaps and resets disconnected, with an accessible data alternative', async () => {
  const points = [0, 1, 2, 3].map((time) => ({
    metric: 'test',
    collectedMs: time * 1000,
    value: time === 1 ? null : 2,
    state: time === 1 ? 'counterReset' : 'ok',
  }));
  expect(
    chartPath(points, { start: 0, end: 3000, cadence: 1000 }).match(/M/g),
  ).toHaveLength(2);
  render(
    <MetricChart
      title="Test"
      points={points}
      range={{ start: 0, end: 3000, cadence: 1000 }}
    />,
  );
  const details = screen.getByText('Data table for Test').closest('details');
  if (!details) throw new Error('details');
  details.open = true;
  fireEvent(details, new Event('toggle'));
  const table = await screen.findByRole('table');
  expect(within(table).getByText('—')).toBeInTheDocument();
  expect(within(table).getByText(/Statistics reset/)).toBeInTheDocument();
});
it('copies setup guidance without executing enabling SQL', async () => {
  const writeText = vi.fn(() => Promise.resolve());
  Object.assign(navigator, { clipboard: { writeText } });
  const reply = monitorReply();
  const status = reply.sources.find((item) => item.source === 'statements');
  if (!status) throw new Error('fixture');
  status.state = {
    kind: 'unavailable',
    reason: 'pg_stat_statements is missing',
  };
  render(
    <SourcePanel
      panel="statements"
      profile={monitorProfile}
      database="orders"
      sessionId="s"
      onOpenSql={vi.fn()}
      monitor={{
        data: { reply, tables: {}, points: [] },
        now: 1000,
        paused: false,
        setPaused: vi.fn(),
        interval: 5000,
        setInterval: vi.fn(),
        error: null,
      }}
    />,
  );
  await userEvent.click(
    screen.getByRole('button', { name: 'Copy setup guidance' }),
  );
  expect(writeText).toHaveBeenCalledWith(
    'CREATE EXTENSION pg_stat_statements;',
  );
  expect(explainQuery).not.toHaveBeenCalled();
});
it('requests a plan only after review and clears it when the monitor closes', async () => {
  vi.mocked(explainQuery).mockResolvedValue({
    kind: 'blocked',
    message: 'test-only',
  });
  const mounted = render(
    <TopQueries
      table={decodeTable(monitorBytes('statements'))}
      sessionId="s"
      onOpenSql={vi.fn()}
    />,
  );
  await userEvent.click(screen.getByRole('gridcell', { name: 'SELECT ?' }));
  await userEvent.click(screen.getByRole('button', { name: 'Explain' }));
  await userEvent.click(
    screen.getByRole('switch', { name: 'Statement reviewed' }),
  );
  await userEvent.click(
    screen.getByRole('button', { name: 'Explain plan only' }),
  );
  expect(explainQuery).toHaveBeenCalledWith(
    expect.objectContaining({ analyze: false, sql: 'SELECT ?' }),
  );
  mounted.unmount();
});
it('opens normalised SQL for review and never explains before explicit review', async () => {
  const onOpenSql = vi.fn();
  render(
    <TopQueries
      table={decodeTable(monitorBytes('statements'))}
      sessionId="s"
      onOpenSql={onOpenSql}
    />,
  );
  await userEvent.click(screen.getByRole('gridcell', { name: 'SELECT ?' }));
  await userEvent.click(
    screen.getByRole('button', { name: 'Open in editor for review' }),
  );
  expect(onOpenSql).toHaveBeenCalledWith(expect.stringContaining('-- Review'));
  await userEvent.click(screen.getByRole('button', { name: 'Explain' }));
  expect(
    screen.getByRole('button', { name: 'Explain plan only' }),
  ).toBeDisabled();
  expect(explainQuery).not.toHaveBeenCalled();
});
