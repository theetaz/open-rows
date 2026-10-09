// SPDX-License-Identifier: Apache-2.0
import { afterEach, expect, it, vi } from 'vitest';
import type { UiReport } from '@openrows/ipc';
import { SLOW_IPC_MS, timeIpc } from './devMonitor';

afterEach(() => {
  vi.restoreAllMocks();
});

it('logs only the IPC commands that take at least the threshold', async () => {
  let clock = 0;
  vi.spyOn(performance, 'now').mockImplementation(() => clock);
  const internals = {
    invoke: vi.fn((command: string) => {
      clock += command === 'slow_command' ? SLOW_IPC_MS : 1;
      return Promise.resolve('ok');
    }),
  };
  const reports: UiReport[] = [];
  timeIpc(internals, (report) => reports.push(report));
  await expect(internals.invoke('fast_command')).resolves.toBe('ok');
  await expect(internals.invoke('slow_command')).resolves.toBe('ok');
  expect(reports).toEqual([
    {
      kind: 'slowIpc',
      name: 'slow_command',
      message: null,
      durationMs: SLOW_IPC_MS,
    },
  ]);
});
