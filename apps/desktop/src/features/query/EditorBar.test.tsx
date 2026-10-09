// SPDX-License-Identifier: Apache-2.0
import { screen, within } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { choose, fakeRuns, renderTab } from './testQueryTab';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('./QueryEditor', async () => ({
  QueryEditor: (await import('./testEditor')).TextareaEditor,
}));

const editorText = (): string =>
  screen.getByRole<HTMLTextAreaElement>('textbox', { name: 'SQL editor' })
    .value;

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
  expect(editorText()).toMatch(/^-- uglify postgres 2 spaces upper standard\n/);
  await user.click(screen.getByRole('button', { name: 'Format options' }));
  await user.click(screen.getByRole('menuitem', { name: /Indent Width/ }));
  await user.click(screen.getByRole('menuitemradio', { name: /4 spaces/ }));
  await user.click(screen.getByRole('button', { name: 'Beautify' }));
  expect(editorText()).toMatch(
    /^-- beautify postgres 4 spaces upper standard\n/,
  );
  await user.click(screen.getByRole('textbox', { name: 'SQL editor' }));
  await user.keyboard('{Meta>}{Shift>}i{/Shift}{/Meta}');
  expect(editorText()).toMatch(/^-- uglify postgres 4 spaces upper standard\n/);
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
