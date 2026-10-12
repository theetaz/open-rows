// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import * as ipc from '@openrows/ipc';
import type { AlertRule, Incident, MonitorIncident } from '@openrows/ipc';
import { IncidentsPanel } from './IncidentsPanel';
import { IncidentNotice } from './IncidentNotice';
import { BackgroundToggle } from './BackgroundToggle';

vi.mock('@openrows/ipc', async () => ({
  ...(await import('../../test/ipcStub')).ipcStub(),
}));
const target = { profileId: 'p', database: 'orders' };
const rule: AlertRule = {
  id: 'waiting-sessions',
  title: 'Waiting sessions',
  metric: 'sessions:waiting',
  comparison: 'above',
  fire: 5,
  clear: 1,
  fireAfter: { samples: 3, ms: 30000 },
  clearAfter: { samples: 3, ms: 30000 },
  cooldownMs: 60000,
  severity: 'warning',
};
const incident = (id: number, endedMs: number | null): Incident => ({
  id,
  rule,
  scope: 'server',
  startedMs: 1_000,
  endedMs,
  onset: [{ wallMs: 1_000, value: 6 }],
  peak: { wallMs: 2_000, value: 12 },
  last: { wallMs: 3_000, value: endedMs === null ? 9 : 0 },
});
let notify: ((event: MonitorIncident) => void) | undefined;
beforeEach(() => {
  notify = undefined;
  vi.mocked(ipc.securityStatus).mockResolvedValue({
    locked: false,
    available: true,
    notice: null,
  });
  vi.mocked(ipc.onSecurityChanged).mockResolvedValue(() => undefined);
  vi.mocked(ipc.onMonitorIncident).mockImplementation((handler) => {
    notify = handler;
    return Promise.resolve(() => undefined);
  });
  vi.mocked(ipc.monitorIncidents).mockResolvedValue({
    rules: [rule],
    incidents: [incident(2, null), incident(1, 4_000)],
    background: false,
  });
});

it('lists open and resolved incidents with rule, times and evidence', async () => {
  render(<IncidentsPanel target={target} revision="1" />);
  const table = await screen.findByRole('table', { name: 'Incident list' });
  const rows = within(table).getAllByRole('row');
  expect(rows).toHaveLength(3);
  expect(rows[1]).toHaveTextContent('Open');
  expect(rows[1]).toHaveTextContent('onset 6 → peak 12 → last 9');
  expect(rows[1]).toHaveTextContent('fires ≥ 5 · clears ≤ 1');
  expect(rows[2]).toHaveTextContent('Resolved');
  expect(rows[2]).toHaveTextContent('recovered 0');
  expect(screen.getByText('Rules (1)')).toBeInTheDocument();
  vi.mocked(ipc.monitorIncidents).mockClear();
  act(() => {
    notify?.({
      target: { profileId: 'P', database: 'orders' },
      transition: { kind: 'resolved', incident: incident(2, 5_000) },
    });
  });
  expect(ipc.monitorIncidents).toHaveBeenCalledWith(target);
});

it('shows in-app notifications only and dismisses them', async () => {
  render(<IncidentNotice />);
  await vi.waitFor(() => {
    expect(notify).toBeDefined();
  });
  act(() => {
    notify?.({
      target,
      transition: { kind: 'opened', incident: incident(3, null) },
    });
  });
  const notice = await screen.findByRole('status', {
    name: 'Monitor incidents',
  });
  expect(notice).toHaveTextContent(
    'Incident opened: Waiting sessions on orders (9)',
  );
  await userEvent.click(
    within(notice).getByRole('button', { name: 'Dismiss' }),
  );
  expect(
    screen.queryByRole('status', { name: 'Monitor incidents' }),
  ).toBeNull();
});

it('opts in to background monitoring explicitly', async () => {
  render(<BackgroundToggle target={target} enabled={false} disabled={false} />);
  await userEvent.click(screen.getByRole('checkbox', { name: 'Background' }));
  expect(ipc.setMonitorBackground).toHaveBeenCalledWith(target, true);
  expect(screen.queryByRole('alert')).toBeNull();
});
