// SPDX-License-Identifier: Apache-2.0
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { ScheduleRequest } from '@openrows/ipc';
import { InvestigationSchedulePane } from './InvestigationSchedulePane';
import { monitorProfile } from './monitorFixture';
const { schedule, list } = vi.hoisted(() => ({
  schedule: vi
    .fn<(request: ScheduleRequest) => Promise<void>>()
    .mockResolvedValue(undefined),
  list: vi.fn().mockResolvedValue([]),
}));
vi.mock('@openrows/ipc', () => ({
  scheduleInvestigation: schedule,
  listInvestigationSchedules: list,
  cancelInvestigationSchedule: vi.fn(),
  importBoard: vi.fn(),
  saveReport: vi.fn(),
}));
vi.mock('../settings/ai/aiSettings', () => ({
  aiSettings: {
    useValue: () => ({
      enabled: true,
      providers: [{ id: 'fixture', name: 'Fixture' }],
    }),
  },
}));
describe('scheduled investigations', () => {
  it('mounts with no model calls and requires explicit provider and model before enabling', async () => {
    render(
      <InvestigationSchedulePane profile={monitorProfile} database="fixture" />,
    );
    expect(schedule).not.toHaveBeenCalled();
    expect(list).not.toHaveBeenCalled();
    expect(
      screen.getByRole('button', { name: 'Enable scheduled investigation' }),
    ).toBeDisabled();
    await userEvent.selectOptions(
      screen.getByLabelText('Scheduled provider'),
      'fixture',
    );
    await userEvent.type(
      screen.getByLabelText('Scheduled model'),
      'fixture-model',
    );
    await userEvent.clear(
      screen.getByLabelText('Maximum price millionths per million tokens'),
    );
    await userEvent.type(
      screen.getByLabelText('Maximum price millionths per million tokens'),
      '10000000',
    );
    await userEvent.click(
      screen.getByRole('button', { name: 'Enable scheduled investigation' }),
    );
    expect(schedule).toHaveBeenCalledOnce();
    expect(schedule.mock.calls[0]?.[0]).toMatchObject({
      config: {
        enabled: true,
        provider: 'fixture',
        model: 'fixture-model',
        maxRuns: 4,
      },
      request: {
        monitoring: true,
        query: null,
        connection: monitorProfile.id,
      },
    });
  });
});
