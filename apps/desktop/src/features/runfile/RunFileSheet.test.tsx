// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import {
  cancelQuery,
  checkRunFile,
  pickRunFile,
  startRunFile,
} from '@openrows/ipc';
import type { RunFileCheckDto, RunFileEventDto } from '@openrows/ipc';
import { JobsButton } from '../jobs';
import { RunFileHost } from './RunFileHost';
import { pickRunFileFor } from './runFileJob';
import { showRunFile } from './runFileStore';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const file = { path: '/tmp/dump.sql', name: 'dump.sql', bytes: 2_097_152 };
const target = {
  sessionId: 'session-1',
  database: 'shop',
  connection: 'Local',
};

function check(change: Partial<RunFileCheckDto> = {}): RunFileCheckDto {
  return {
    bytes: file.bytes,
    statements: 40,
    gate: 'run',
    confirm: 0,
    blocked: 0,
    samples: [],
    confirmToken: null,
    transactions: true,
    ...change,
  };
}

let emit: (event: RunFileEventDto) => void = () => undefined;

beforeEach(() => {
  vi.mocked(startRunFile).mockImplementation((_request, onEvent) => {
    emit = onEvent;
    return Promise.resolve({ runId: 'run-1' });
  });
});

async function open(result: RunFileCheckDto): Promise<void> {
  vi.mocked(checkRunFile).mockResolvedValueOnce(result);
  vi.mocked(pickRunFile).mockResolvedValueOnce(file);
  render(
    <ThemeProvider>
      <JobsButton />
      <RunFileHost />
    </ThemeProvider>,
  );
  await act(async () => {
    await pickRunFileFor(target);
  });
}

/** Open the sheet for a file that needs confirmation and start it. */
async function startConfirmed(
  user: ReturnType<typeof userEvent.setup>,
): Promise<void> {
  await open(
    check({
      gate: 'confirm',
      confirm: 2,
      confirmToken: 'token-1',
      samples: [{ line: 3, preview: 'DROP TABLE old', blocked: false }],
    }),
  );
  const sheet = await screen.findByRole('dialog', { name: 'Run dump.sql' });
  expect(await within(sheet).findByText('DROP TABLE old')).toBeInTheDocument();
  await user.click(
    within(sheet).getByRole('radio', { name: 'Continue and list every error' }),
  );
  await user.click(within(sheet).getByRole('button', { name: 'Run Anyway' }));
}

it('checks the file and starts it after confirmation with the chosen error policy', async () => {
  const user = userEvent.setup();
  await startConfirmed(user);
  expect(checkRunFile).toHaveBeenCalledWith({
    sessionId: 'session-1',
    path: '/tmp/dump.sql',
  });
  expect(startRunFile).toHaveBeenCalledWith(
    {
      sessionId: 'session-1',
      path: '/tmp/dump.sql',
      stopOnError: false,
      transaction: false,
      confirmToken: 'token-1',
    },
    expect.any(Function),
  );
  const progress = await screen.findByRole('dialog', { name: 'Run dump.sql' });
  act(() => {
    emit({
      type: 'progress',
      statements: 12,
      bytes: 1_048_576,
      errors: 0,
      current: 'INSERT INTO t',
    });
  });
  expect(within(progress).getByText('12 of 40 statements')).toBeInTheDocument();
  expect(within(progress).getByText('1.0 MB of 2.0 MB')).toBeInTheDocument();
  expect(within(progress).getByText('INSERT INTO t')).toBeInTheDocument();
  await user.click(
    within(progress).getByRole('button', { name: 'Cancel Run' }),
  );
  expect(cancelQuery).toHaveBeenCalledWith('run-1');
  act(() => {
    showRunFile(null);
  });
});

it('lists errors with their lines, sums up the run and reopens from the jobs popover', async () => {
  const user = userEvent.setup();
  await startConfirmed(user);
  const progress = await screen.findByRole('dialog', { name: 'Run dump.sql' });
  act(() => {
    emit({
      type: 'error',
      statement: 13,
      line: 57,
      preview: 'INSERT INTO missing',
      message: 'relation "missing" does not exist',
    });
  });
  const errors = within(progress).getByRole('region', { name: 'Errors' });
  expect(within(errors).getByText('Line 57')).toBeInTheDocument();
  expect(
    within(errors).getByText('relation "missing" does not exist'),
  ).toBeInTheDocument();
  act(() => {
    emit({
      type: 'finished',
      outcome: 'failed',
      statements: 40,
      errors: 1,
      rowsAffected: 39,
      rolledBack: false,
      message: null,
      elapsedMs: 65_000,
    });
  });
  expect(within(progress).getByRole('alert')).toHaveTextContent(
    'Stopped with errors: 40 of 40 statements ran in 1:05.',
  );
  await user.click(within(progress).getByRole('button', { name: 'Done' }));
  expect(screen.queryByRole('dialog', { name: 'Run dump.sql' })).toBeNull();
  await user.click(screen.getByRole('button', { name: /^Background jobs/ }));
  const [job] = await screen.findAllByRole('listitem', {
    name: 'Run dump.sql',
  });
  if (!job) throw new Error('no job');
  await user.click(within(job).getByRole('button', { name: 'Details…' }));
  expect(
    await screen.findByRole('region', { name: 'Errors' }),
  ).toBeInTheDocument();
  act(() => {
    showRunFile(null);
  });
});

it('blocks every run on a read-only connection when a statement writes', async () => {
  await open(
    check({
      gate: 'block',
      blocked: 1,
      samples: [{ line: 9, preview: 'DELETE FROM t', blocked: true }],
    }),
  );
  const sheet = await screen.findByRole('dialog', { name: 'Run dump.sql' });
  expect(await within(sheet).findByRole('alert')).toHaveTextContent(
    /read-only/,
  );
  expect(within(sheet).getByRole('button', { name: 'Run' })).toBeDisabled();
  act(() => {
    showRunFile(null);
  });
});

it('offers no transaction where the engine commits DDL implicitly', async () => {
  await open(check({ transactions: false }));
  const mysql = await screen.findByRole('dialog', { name: 'Run dump.sql' });
  expect(
    await within(mysql).findByRole('checkbox', { name: /one transaction/ }),
  ).toBeDisabled();
  expect(
    within(mysql).getByText(/commits schema changes immediately/),
  ).toBeInTheDocument();
  act(() => {
    showRunFile(null);
  });
});

it('runs in one transaction, which always stops at the first error', async () => {
  const user = userEvent.setup();
  await open(check());
  const sheet = await screen.findByRole('dialog', { name: 'Run dump.sql' });
  await user.click(
    await within(sheet).findByRole('checkbox', { name: /one transaction/ }),
  );
  expect(
    within(sheet).getByRole('radio', { name: 'Continue and list every error' }),
  ).toBeDisabled();
  await user.click(within(sheet).getByRole('button', { name: 'Run' }));
  expect(startRunFile).toHaveBeenLastCalledWith(
    expect.objectContaining({
      stopOnError: true,
      transaction: true,
      confirmToken: null,
    }),
    expect.any(Function),
  );
  act(() => {
    showRunFile(null);
  });
});
