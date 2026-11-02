// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ScheduledJob } from '@openrows/ipc';
import { ScheduleResults } from './ScheduleResults';
import { monitorProfile } from './monitorFixture';
const { cancel, pause, run } = vi.hoisted(() => ({
  cancel: vi.fn().mockResolvedValue(undefined),
  pause: vi.fn().mockResolvedValue(undefined),
  run: vi.fn().mockResolvedValue(undefined),
}));
vi.mock('@openrows/ipc', () => ({
  cancelInvestigationSchedule: cancel,
  pauseInvestigationSchedule: pause,
  runInvestigationSchedule: run,
  importBoard: vi.fn(),
  saveReport: vi.fn(),
}));
beforeEach(() => {
  vi.clearAllMocks();
});
function job(overrides: Partial<ScheduledJob>): ScheduledJob {
  return {
    id: 'job',
    connection: monitorProfile.id,
    database: 'fixture',
    config: {
      enabled: true,
      provider: 'fixture',
      model: 'fixture-model',
      intervalSecs: 900,
      maxRuns: 4,
      steps: 12,
      tokens: 32768,
      timeoutSecs: 60,
      maxCostMicros: 1000000,
      pricePerMillionMicros: 1000000,
      tools: ['read_health_snapshot'],
    },
    state: 'waiting',
    pause: null,
    runs: 0,
    failure: null,
    results: [],
    ...overrides,
  };
}
function mount(jobs: ScheduledJob[]): {
  refresh: () => Promise<void>;
  setStatus: (value: string) => void;
} {
  const handlers = {
    refresh: vi.fn().mockResolvedValue(undefined),
    setStatus: vi.fn(),
  };
  render(
    <ScheduleResults
      profile={monitorProfile}
      database="fixture"
      jobs={jobs}
      {...handlers}
    />,
  );
  return handlers;
}
describe('scheduled investigation controls', () => {
  it('runs now, pauses and cancels a waiting schedule', async () => {
    const { refresh } = mount([job({})]);
    expect(
      screen.getByText(/Waiting for the next interval · 0\/4 runs/),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Run now' }));
    expect(run).toHaveBeenCalledWith('job');
    await userEvent.click(
      screen.getByRole('button', { name: 'Pause schedule' }),
    );
    expect(pause).toHaveBeenCalledWith('job', true);
    await userEvent.click(
      screen.getByRole('button', { name: 'Cancel schedule' }),
    );
    expect(cancel).toHaveBeenCalledWith('job');
    await waitFor(() => {
      expect(refresh).toHaveBeenCalledTimes(3);
    });
  });
  it('resumes a user pause and explains lock and AI-off pauses', async () => {
    mount([
      job({ state: 'paused', pause: 'user' }),
      job({ id: 'locked', state: 'paused', pause: 'locked' }),
      job({ id: 'off', state: 'paused', pause: 'aiOff' }),
    ]);
    expect(
      screen.getByText(/Paused while the app is locked/),
    ).toBeInTheDocument();
    expect(screen.getByText(/Paused while AI is off/)).toBeInTheDocument();
    for (const button of screen.getAllByRole('button', { name: 'Run now' }))
      expect(button).toBeDisabled();
    await userEvent.click(
      screen.getByRole('button', { name: 'Resume schedule' }),
    );
    expect(pause).toHaveBeenCalledWith('job', false);
  });
  it('shows why a run could not start and offers removal after the schedule ends', async () => {
    run.mockRejectedValueOnce(new Error('Turn AI on in Settings first'));
    const { setStatus } = mount([
      job({}),
      job({
        id: 'ended',
        state: 'stopped',
        runs: 1,
        failure: 'The last run was refused',
      }),
    ]);
    await userEvent.click(
      screen.getAllByRole('button', { name: 'Run now' })[0] as HTMLElement,
    );
    await waitFor(() => {
      expect(setStatus).toHaveBeenCalledWith('Turn AI on in Settings first');
    });
    expect(screen.getByRole('alert')).toHaveTextContent(
      'The last run was refused',
    );
    expect(
      screen.getByRole('button', { name: 'Remove schedule' }),
    ).toBeEnabled();
  });
});
