// SPDX-License-Identifier: Apache-2.0
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import * as ipc from '@openrows/ipc';
import type { MonitorChanged, MonitorWindow } from '@openrows/ipc';
import { MonitorTab } from './MonitorTab';
import { monitorBytes, monitorProfile, monitorReply } from './monitorFixture';
import { monitorSettings } from '../settings/monitorSettings';

vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../test/ipcStub')).ipcStub(),
  startMonitor: vi.fn(),
  stopMonitor: vi.fn(() => Promise.resolve()),
  pauseMonitor: vi.fn(() => Promise.resolve()),
  monitorSnapshot: vi.fn(),
  monitorWindow: vi.fn(),
  topStatements: vi.fn(),
  onMonitorChanged: vi.fn(() => Promise.resolve(() => undefined)),
  monitorHistory: vi.fn(() => Promise.resolve([])),
  configureMonitorHistory: vi.fn(() => Promise.resolve()),
  purgeMonitorHistory: vi.fn(() => Promise.resolve()),
}));
vi.mock('./monitorDecoder', async () => {
  const { decodeTable } = await import('./monitorArrow');
  return {
    MonitorDecoder: class {
      decode(bytes: ArrayBuffer) {
        return Promise.resolve(decodeTable(bytes));
      }
      stop() {
        /* Real worker termination is verified in monitorDecoder.test.ts. */
      }
    },
  };
});
const props = {
  profile: monitorProfile,
  database: 'orders',
  connected: true,
  sessionId: 's',
  onOpenSql: vi.fn(),
};
beforeEach(() => {
  monitorSettings.set({ enabledProfiles: [] });
  vi.mocked(ipc.startMonitor).mockResolvedValue(monitorReply());
  vi.mocked(ipc.monitorSnapshot).mockResolvedValue(monitorReply());
  vi.mocked(ipc.monitorWindow).mockImplementation((request: MonitorWindow) =>
    Promise.resolve(monitorBytes(request.source)),
  );
  vi.mocked(ipc.topStatements).mockResolvedValue(monitorBytes('statements'));
  vi.mocked(ipc.securityStatus).mockResolvedValue({
    locked: false,
    available: true,
    notice: null,
  });
  vi.mocked(ipc.onSecurityChanged).mockResolvedValue(() => undefined);
});
function view() {
  return render(<MonitorTab {...props} />);
}
async function loaded() {
  await screen.findByRole('region', { name: 'Monitor on orders' });
  await waitFor(() => {
    expect(ipc.monitorWindow).toHaveBeenCalled();
  });
  await screen.findByText('2 count');
}

it('shows real overview values and unknown/reset/gap states without zero fallbacks', async () => {
  view();
  await loaded();
  expect(
    within(screen.getByRole('article', { name: 'Transactions/s' })).getByText(
      'Unknown',
    ),
  ).toBeInTheDocument();
  expect(screen.getAllByText(/Statistics reset/).length).toBeGreaterThan(0);
  expect(
    screen.getByRole('img', { name: /Active sessions trend/ }),
  ).toBeInTheDocument();
  expect(
    screen.getByLabelText('Monitor time window').querySelectorAll('option'),
  ).toHaveLength(1);
});

it('renders every source panel with sorted tables, NULL states and blocking direction', async () => {
  view();
  await loaded();
  for (const name of [
    'Sessions',
    'Blocking',
    'Top queries',
    'Tables',
    'Indexes',
    'Replication',
    'I/O',
  ]) {
    await userEvent.click(screen.getByRole('radio', { name }));
    expect(screen.getByRole('heading', { level: 2 })).toBeInTheDocument();
  }
  await userEvent.click(screen.getByRole('radio', { name: 'Blocking' }));
  const tree = screen.getByLabelText('Blocking chains');
  expect(tree).toHaveTextContent('PID 7');
  expect(tree).toHaveTextContent('PID 8');
  expect(tree).toHaveTextContent('Lock:transactionid');
  await userEvent.click(screen.getByRole('radio', { name: 'Tables' }));
  expect(
    screen.getByRole('gridcell', { name: /Statistics reset/ }),
  ).toBeInTheDocument();
  expect(screen.getAllByRole('gridcell', { name: 'Unknown' })).toHaveLength(2);
});

it('pauses, respects selected intervals, resumes and stops when the tab closes', async () => {
  const mounted = view();
  await loaded();
  await userEvent.click(screen.getByRole('button', { name: 'Pause monitor' }));
  await waitFor(() => {
    expect(ipc.stopMonitor).toHaveBeenCalled();
  });
  expect(
    screen.getByRole('button', { name: 'Resume monitor' }),
  ).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText('Monitor interval'), {
    target: { value: '2000' },
  });
  await waitFor(() => {
    expect(ipc.startMonitor).toHaveBeenLastCalledWith({
      target: { profileId: props.profile.id, database: 'orders' },
      intervalMs: 2000,
    });
  });
  vi.mocked(ipc.stopMonitor).mockClear();
  mounted.unmount();
  await waitFor(() => {
    expect(ipc.stopMonitor).toHaveBeenCalledWith({
      profileId: props.profile.id,
      database: 'orders',
    });
  });
});

it('hides data and stops on lock; unlock needs an explicit resume', async () => {
  let changed: ((locked: boolean) => void) | undefined;
  vi.mocked(ipc.onSecurityChanged).mockImplementation((callback) => {
    changed = callback;
    return Promise.resolve(() => undefined);
  });
  view();
  await loaded();
  vi.mocked(ipc.securityStatus).mockResolvedValue({
    locked: true,
    available: true,
    notice: null,
  });
  await act(async () => {
    changed?.(true);
    await Promise.resolve();
  });
  expect(screen.queryByText('2 count')).not.toBeInTheDocument();
  expect(screen.getByRole('status')).toHaveTextContent('hidden');
  await waitFor(() => {
    expect(ipc.stopMonitor).toHaveBeenCalled();
  });
  vi.mocked(ipc.startMonitor).mockClear();
  vi.mocked(ipc.securityStatus).mockResolvedValue({
    locked: false,
    available: true,
    notice: null,
  });
  await act(async () => {
    changed?.(false);
    await Promise.resolve();
  });
  expect(
    screen.getByRole('button', { name: 'Resume monitor' }),
  ).toBeInTheDocument();
  expect(ipc.startMonitor).not.toHaveBeenCalled();
});

it('disconnect hides prior observations and unsupported engines never start collectors', async () => {
  const mounted = view();
  await loaded();
  mounted.rerender(<MonitorTab {...props} connected={false} />);
  expect(screen.queryByText('2 count')).not.toBeInTheDocument();
  expect(screen.getByRole('status')).toHaveTextContent('Disconnected');
  mounted.unmount();
  vi.mocked(ipc.startMonitor).mockClear();
  render(
    <MonitorTab {...props} profile={{ ...props.profile, engine: 'duckdb' }} />,
  );
  expect(
    await screen.findByText(/Monitoring for duckdb is coming/),
  ).toBeInTheDocument();
  expect(ipc.startMonitor).not.toHaveBeenCalled();
});

it('ignores events for other generations and scopes', async () => {
  let changed: ((event: MonitorChanged) => void) | undefined;
  vi.mocked(ipc.onMonitorChanged).mockImplementation((callback) => {
    changed = callback;
    return Promise.resolve(() => undefined);
  });
  view();
  await loaded();
  vi.mocked(ipc.monitorSnapshot).mockClear();
  await act(async () => {
    changed?.({
      target: { profileId: 'other', database: 'orders' },
      generation: '1',
      sources: [],
    });
    changed?.({
      target: { profileId: props.profile.id, database: 'orders' },
      generation: 'old',
      sources: [],
    });
    await Promise.resolve();
  });
  expect(ipc.monitorSnapshot).not.toHaveBeenCalled();
});

it.each(['sqlite', 'clickhouse', 'libsql', 'libsql-file', 'duckdb'] as const)(
  'shows the explicit coming state for %s without collector admission',
  async (engine) => {
    render(<MonitorTab {...props} profile={{ ...props.profile, engine }} />);
    expect(
      await screen.findByText(
        `Monitoring for ${engine} is coming. This engine is not supported by the current collector.`,
      ),
    ).toBeInTheDocument();
    expect(ipc.startMonitor).not.toHaveBeenCalled();
  },
);
it('cancels pending admission and rejects late samples after tab close', async () => {
  let finish: ((reply: ReturnType<typeof monitorReply>) => void) | undefined;
  vi.mocked(ipc.startMonitor).mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  const mounted = view();
  await waitFor(() => {
    expect(ipc.startMonitor).toHaveBeenCalled();
  });
  mounted.unmount();
  await act(async () => {
    finish?.(monitorReply());
    await Promise.resolve();
  });
  expect(ipc.stopMonitor).toHaveBeenCalled();
  expect(ipc.monitorWindow).not.toHaveBeenCalled();
});

it.each(['mysql', 'mariadb'] as const)(
  'starts %s monitoring with server labels and explicit disabled/denied states',
  async (engine) => {
    const reply = monitorReply();
    const statement = reply.capabilities.find((d) => d.source === 'statements');
    if (statement)
      statement.state = {
        kind: 'unavailable',
        reason: 'performance_schema disabled; restart required',
      };
    const activity = reply.capabilities.find((d) => d.source === 'activity');
    if (activity)
      activity.state = { kind: 'permissionDenied', reason: 'PROCESS required' };
    const activityStatus = reply.sources.find((d) => d.source === 'activity');
    if (activityStatus)
      activityStatus.state = {
        kind: 'permissionDenied',
        reason: 'PROCESS required',
      };
    const status = reply.sources.find((d) => d.source === 'statements');
    if (status)
      status.state = {
        kind: 'unavailable',
        reason: 'performance_schema disabled; restart required',
      };
    vi.mocked(ipc.startMonitor).mockResolvedValue(reply);
    vi.mocked(ipc.monitorSnapshot).mockResolvedValue(reply);
    render(<MonitorTab {...props} profile={{ ...props.profile, engine }} />);
    await screen.findByRole('article', { name: 'Queries/s' });
    expect(ipc.startMonitor).toHaveBeenCalled();
    expect(
      screen.getByText(engine === 'mysql' ? /· MySQL$/ : /· MariaDB$/),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('article', { name: 'InnoDB buffer pool hit ratio' }),
    ).toBeInTheDocument();
    for (const name of ['Table I/O', 'Index I/O', 'File I/O'])
      expect(screen.getByRole('radio', { name })).toBeInTheDocument();
    expect(
      (await screen.findAllByText(/PROCESS required/)).length,
    ).toBeGreaterThan(0);
    expect(
      screen.queryByRole('article', { name: 'Transactions/s' }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('radio', { name: 'Top queries' }));
    expect(
      screen.getAllByText(/performance_schema disabled/).length,
    ).toBeGreaterThan(0);
  },
);

it('starts SQL Server monitoring with counter tiles, wait panel and restricted DMV state', async () => {
  const reply = monitorReply();
  const reason =
    'Requires VIEW SERVER STATE (or VIEW SERVER PERFORMANCE STATE); ask the DBA to review it, no grants are changed';
  for (const item of [...reply.capabilities, ...reply.sources])
    if (item.source === 'activity')
      item.state = { kind: 'permissionDenied', reason };
  vi.mocked(ipc.startMonitor).mockResolvedValue(reply);
  vi.mocked(ipc.monitorSnapshot).mockResolvedValue(reply);
  render(
    <MonitorTab
      {...props}
      profile={{ ...props.profile, engine: 'sqlserver' }}
    />,
  );
  await screen.findByRole('article', { name: 'Batch requests/s' });
  expect(ipc.startMonitor).toHaveBeenCalled();
  expect(screen.getByText(/· SQL Server$/)).toBeInTheDocument();
  for (const name of ['Page life expectancy', 'Buffer cache hit ratio'])
    expect(screen.getByRole('article', { name })).toBeInTheDocument();
  for (const name of ['Table usage', 'Index usage', 'Waits'])
    expect(screen.getByRole('radio', { name })).toBeInTheDocument();
  expect(
    (await screen.findAllByText(/VIEW SERVER STATE/)).length,
  ).toBeGreaterThan(0);
  expect(
    screen.queryByRole('article', { name: 'Transactions/s' }),
  ).not.toBeInTheDocument();
});
