// SPDX-License-Identifier: Apache-2.0
import { act } from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { runMemorySpike } from './memory';
import type { SpikeSession } from './lifecycle';

const mocks = vi.hoisted(() => ({
  create: vi.fn(),
  connect: vi.fn(),
  session: vi.fn(),
  listen: vi.fn(),
  unlisten: vi.fn(),
  monitor: vi.fn(),
}));
vi.mock('@openrows/ipc', () => ({
  createProfile: mocks.create,
  openConnection: mocks.connect,
  openSession: mocks.session,
  onMonitorChanged: mocks.listen,
}));
vi.mock('../features/monitor/MonitorTab', () => ({
  MonitorTab: (props: { database: string; sessionId: string }) => {
    mocks.monitor(props);
    return <p>Monitor {props.database}</p>;
  },
}));
const profile = { id: 'p1', engine: 'sqlite' };
beforeEach(() => {
  vi.resetAllMocks();
  mocks.create.mockResolvedValue(profile);
  mocks.connect.mockResolvedValue({});
  mocks.session.mockResolvedValue({
    sessionId: 's1',
    server: { database: 'main' },
  });
});
afterEach(() => {
  document.body.replaceChildren();
});

function session(): {
  spike: SpikeSession;
  progress: ReturnType<typeof vi.fn>;
} {
  const progress = vi.fn();
  return { spike: { progress } as unknown as SpikeSession, progress };
}

it('connects a disposable in-memory SQLite profile and reports readiness', async () => {
  const { spike, progress } = session();
  await runMemorySpike(spike, 'connected');
  expect(mocks.create).toHaveBeenCalledWith(
    expect.objectContaining({
      engine: 'sqlite',
      file: expect.objectContaining({ path: '', memory: true }) as unknown,
      passwordStorage: 'none',
    }),
  );
  expect(mocks.session).toHaveBeenCalledWith('p1', null);
  expect(mocks.monitor).not.toHaveBeenCalled();
  expect(document.body.childElementCount).toBe(1);
  expect(progress).toHaveBeenLastCalledWith('memory-ready', 'connected');
});

it('opens the Monitor and waits for its first collector sample', async () => {
  let emit: (event: { target: { profileId: string } }) => void = () => {};
  mocks.listen.mockImplementation((handler: typeof emit) => {
    emit = handler;
    return Promise.resolve(mocks.unlisten);
  });
  const { spike, progress } = session();
  const running = runMemorySpike(spike, 'monitor');
  // The overlay host is appended just before the Monitor renders into it.
  await vi.waitFor(() => {
    expect(document.body.childElementCount).toBe(1);
  });
  await act(async () => {
    await Promise.resolve();
  });
  expect(mocks.monitor).toHaveBeenCalled();
  expect(progress).not.toHaveBeenCalledWith('memory-ready', 'monitor');
  emit({ target: { profileId: 'other' } });
  emit({ target: { profileId: 'p1' } });
  await running;
  expect(mocks.monitor).toHaveBeenCalledWith(
    expect.objectContaining({ database: 'main', sessionId: 's1' }),
  );
  expect(mocks.unlisten).toHaveBeenCalledOnce();
  expect(progress).toHaveBeenLastCalledWith('memory-ready', 'monitor');
});
