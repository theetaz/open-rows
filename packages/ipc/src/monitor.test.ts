// SPDX-License-Identifier: Apache-2.0
import { beforeEach, describe, expect, it, vi } from 'vitest';
const { invoke } = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: unknown) => Promise<unknown>>(),
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => true }));
import {
  monitorCapabilities,
  monitorSnapshot,
  monitorWindow,
  pauseMonitor,
  startMonitor,
  stopMonitor,
  topStatements,
  monitorHistory,
  configureMonitorHistory,
  purgeMonitorHistory,
} from './monitor';
const target = { profileId: 'p', database: 'db' };
describe('monitoring IPC', () => {
  beforeEach(() => {
    invoke.mockReset();
  });
  it('uses the registered explicit control commands', async () => {
    invoke.mockResolvedValue({ status: 'ok', data: [] });
    await startMonitor({ target, intervalMs: 5000 });
    await pauseMonitor(target);
    await stopMonitor(target);
    await monitorCapabilities(target);
    await monitorSnapshot(target);
    expect(invoke.mock.calls.map(([command]) => command)).toEqual([
      'monitor_start',
      'monitor_pause',
      'monitor_stop',
      'monitor_capabilities',
      'monitor_snapshot',
    ]);
  });
  it('keeps raw windows binary and preserves typed permission states', async () => {
    invoke.mockResolvedValue([1, 2, 3]);
    const request = {
      target,
      source: null,
      limit: 20,
      rowStart: null,
      mean: false,
    };
    expect(new Uint8Array(await monitorWindow(request))).toEqual(
      new Uint8Array([1, 2, 3]),
    );
    await topStatements(request);
    expect(invoke).toHaveBeenLastCalledWith('monitor_top_statements', {
      request,
    });
    invoke.mockRejectedValue({ kind: 'locked', message: 'Locked', code: null });
    await expect(monitorWindow(request)).rejects.toMatchObject({
      kind: 'locked',
    });
  });
  it('uses typed history commands with explicit profile isolation and purge scope', async () => {
    invoke.mockResolvedValue({ status: 'ok', data: [] });
    await monitorHistory(target, 24);
    await configureMonitorHistory(['p']);
    await purgeMonitorHistory('p');
    await purgeMonitorHistory(null);
    expect(invoke.mock.calls).toEqual([
      ['monitor_history', { target, hours: 24 }],
      ['monitor_history_configure', { profiles: ['p'] }],
      ['monitor_history_purge', { profileId: 'p' }],
      ['monitor_history_purge', { profileId: null }],
    ]);
  });
});
