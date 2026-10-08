// SPDX-License-Identifier: Apache-2.0
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { profile } from '../../test/ipcStub';
import { QueryTab } from './QueryTab';
import { idleRun } from './run';
import type { QueryRuns } from './useQueryRuns';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('./QueryEditor', async () => ({
  QueryEditor: (await import('./testEditor')).TextareaEditor,
}));

function fakeRuns(): QueryRuns {
  return {
    runOf: () => idleRun,
    execute: vi.fn(() => Promise.resolve()),
    confirm: vi.fn(),
    dismiss: vi.fn(),
    cancel: vi.fn(),
    release: vi.fn(),
  };
}

function renderTab(runs: QueryRuns = fakeRuns()): {
  user: ReturnType<typeof userEvent.setup>;
  unmount: () => void;
} {
  const user = userEvent.setup();
  const { unmount } = render(
    <QueryTab
      tab={{
        id: 'query-1',
        kind: 'query',
        title: 'SQL Query 1',
        sql: 'select 1',
        connectionId: 'id-local',
        database: 'orders',
      }}
      connection={profile('local')}
      sessionId="session-1"
      notice={null}
      catalog={null}
      runs={runs}
      onSql={vi.fn()}
      onCell={vi.fn()}
      onConnect={vi.fn()}
    />,
  );
  return { user, unmount };
}

const editorText = (): string =>
  screen.getByRole<HTMLTextAreaElement>('textbox', { name: 'SQL editor' })
    .value;

async function choose(
  user: ReturnType<typeof userEvent.setup>,
  button: string,
  item: RegExp,
): Promise<void> {
  await user.click(screen.getByRole('button', { name: button }));
  const menu = screen.getByRole('menu', { name: button });
  const row = Array.from(
    menu.querySelectorAll<HTMLElement>('[role^="menuitem"]'),
  ).find((element) => item.test(element.textContent));
  if (!row) throw new Error(`no ${String(item)} in ${button}`);
  await user.click(row);
}

it('applies the row limit to runs and remembers it', async () => {
  const runs = fakeRuns();
  const { user, unmount } = renderTab(runs);
  expect(screen.getByRole('button', { name: 'Row limit' })).toHaveTextContent(
    'No limit',
  );
  await choose(user, 'Row limit', /1,000 rows$/);
  expect(screen.getByRole('button', { name: 'Row limit' })).toHaveTextContent(
    '1,000 rows',
  );
  await user.click(
    screen.getByRole('button', { name: 'Run current statement' }),
  );
  expect(runs.execute).toHaveBeenCalledWith(
    'query-1',
    expect.objectContaining({ sql: 'select 1', maxRows: 1_000 }),
  );
  unmount();
  renderTab();
  expect(screen.getByRole('button', { name: 'Row limit' })).toHaveTextContent(
    '1,000 rows',
  );
});

it('beautifies and uglifies with the chosen layout options', async () => {
  const { user } = renderTab();
  await choose(user, 'Format options', /Uglify/);
  expect(editorText()).toMatch(/^-- uglify postgres 2 spaces upper\n/);
  await user.click(screen.getByRole('button', { name: 'Format options' }));
  await user.click(screen.getByRole('menuitem', { name: /Indent Width/ }));
  await user.click(screen.getByRole('menuitemradio', { name: /4 spaces/ }));
  await user.click(screen.getByRole('button', { name: 'Beautify' }));
  expect(editorText()).toMatch(/^-- beautify postgres 4 spaces upper\n/);
  await user.click(screen.getByRole('textbox', { name: 'SQL editor' }));
  await user.keyboard('{Meta>}{Shift>}i{/Shift}{/Meta}');
  expect(editorText()).toMatch(/^-- uglify postgres 4 spaces upper\n/);
});

it('opens settings submenus by keyboard and toggles checked options', async () => {
  const { user } = renderTab();
  await user.click(screen.getByRole('button', { name: 'Editor settings' }));
  const fontSize = screen.getByRole('menuitem', { name: /Font Size/ });
  expect(fontSize).toHaveFocus();
  await user.keyboard('{ArrowRight}');
  const sizes = screen.getByRole('menu', { name: 'Font Size' });
  expect(
    within(sizes).getByRole('menuitemradio', { name: /Default/ }),
  ).toHaveFocus();
  expect(
    within(sizes).getByRole('menuitemradio', { name: /Default/ }),
  ).toHaveAttribute('aria-checked', 'true');
  await user.keyboard('{ArrowLeft}');
  expect(screen.queryByRole('menu', { name: 'Font Size' })).toBeNull();
  expect(fontSize).toHaveFocus();
  await user.click(
    screen.getByRole('menuitemcheckbox', { name: /Wrap lines/ }),
  );
  expect(screen.queryByRole('menu', { name: 'Editor settings' })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Editor settings' }));
  expect(
    screen.getByRole('menuitemcheckbox', { name: /Wrap lines/ }),
  ).toHaveAttribute('aria-checked', 'true');
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('menu', { name: 'Editor settings' })).toBeNull();
});
