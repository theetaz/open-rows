// SPDX-License-Identifier: Apache-2.0
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { closeWindow, onWindowClose } from '@openrows/ipc';
import type { QueryTabs } from '../query/useQueryTabs';
import type { QueryRuns } from '../query/useQueryRuns';
import { textValue } from './cellValue';
import { useTableCloseGuard } from './useTableCloseGuard';
import { useTableViews } from './useTableViews';
import type { TableViews } from './useTableViews';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

const tabs = { close: vi.fn() } as unknown as QueryTabs;
const runs = { release: vi.fn() } as unknown as QueryRuns;

/** The guard over real table views, with buttons that stage or start a commit. */
function Harness(): React.JSX.Element {
  const tables = useTableViews();
  const guard = useTableCloseGuard({ tabs, tables, runs });
  return (
    <>
      {guard}
      <button
        type="button"
        onClick={() => {
          stageOne(tables);
        }}
      >
        Stage edit
      </button>
      <button
        type="button"
        onClick={() => {
          tables.setEdits('t', (history) => ({ ...history, pending: true }));
        }}
      >
        Start commit
      </button>
    </>
  );
}

function stageOne(tables: TableViews): void {
  tables.setEdits('t', (history) => ({
    ...history,
    current: [
      {
        id: '{"id":"7"}',
        number: 7,
        kind: 'update',
        original: { id: '7' },
        values: { name: textValue('Ada') },
      },
    ],
  }));
}

/** The close-requested handler the guard registered with the native window. */
function handler(): () => boolean {
  const registered = vi.mocked(onWindowClose).mock.calls[0]?.[0];
  if (!registered) throw new Error('no window close handler');
  return registered;
}

beforeEach(() => {
  vi.mocked(onWindowClose).mockClear();
  vi.mocked(closeWindow).mockClear();
});

it('lets the window close when nothing is staged', async () => {
  render(<Harness />);
  await waitFor(() => {
    expect(onWindowClose).toHaveBeenCalledTimes(1);
  });
  expect(handler()()).toBe(true);
  expect(screen.queryByRole('dialog')).toBeNull();
});

it('asks before quitting with staged edits, then closes the window on Quit', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await waitFor(() => {
    expect(onWindowClose).toHaveBeenCalledTimes(1);
  });
  await user.click(screen.getByRole('button', { name: 'Stage edit' }));
  let allowed: boolean | undefined;
  act(() => {
    allowed = handler()();
  });
  expect(allowed).toBe(false);
  const sheet = await screen.findByRole('dialog', {
    name: 'Quit with staged changes?',
  });
  expect(sheet).toHaveTextContent('1 staged change has not been committed');
  expect(sheet).toHaveTextContent('offered for restore');
  await user.click(screen.getByRole('button', { name: 'Keep editing' }));
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(closeWindow).not.toHaveBeenCalled();
  act(() => {
    handler()();
  });
  await user.click(await screen.findByRole('button', { name: 'Quit' }));
  expect(closeWindow).toHaveBeenCalledTimes(1);
});

it('refuses to quit while a commit is in flight', async () => {
  const user = userEvent.setup();
  render(<Harness />);
  await waitFor(() => {
    expect(onWindowClose).toHaveBeenCalledTimes(1);
  });
  await user.click(screen.getByRole('button', { name: 'Start commit' }));
  act(() => {
    handler()();
  });
  const sheet = await screen.findByRole('dialog', {
    name: 'Commit in progress',
  });
  expect(sheet).toHaveTextContent('Wait until the commit finishes');
  expect(screen.getByRole('button', { name: 'Quit' })).toBeDisabled();
});
