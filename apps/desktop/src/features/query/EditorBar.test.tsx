// SPDX-License-Identifier: Apache-2.0
import { screen, within } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { choose, fakeRuns, renderTab } from './testQueryTab';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);
vi.mock('../assistant/AssistantPanel', () => ({
  AssistantPanel: () => <aside aria-label="Query assistant" />,
}));
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
  await choose(user, 'More query actions', /Uglify/);
  expect(editorText()).toMatch(/^-- uglify postgres 2 spaces upper standard\n/);
  await user.click(screen.getByRole('button', { name: 'More query actions' }));
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

it('keeps Run as the only split button, with Explain, Beautify and AI plain', () => {
  renderTab();
  const bar = within(screen.getByRole('toolbar', { name: 'Query actions' }));
  expect(
    bar
      .getAllByRole('button', { name: /options$/ })
      .map((b) => b.getAttribute('aria-label')),
  ).toEqual(['Run options']);
  for (const name of ['Explain', 'Beautify', 'AI', 'Row limit'])
    expect(bar.getByRole('button', { name })).toBeInTheDocument();
  expect(bar.queryByText('Connect to run')).toBeNull();
});

it('opens settings submenus by keyboard and toggles checked options', async () => {
  const { user } = renderTab();
  await user.click(screen.getByRole('button', { name: 'More query actions' }));
  const fontSize = screen.getByRole('menuitem', { name: /Font Size/ });
  fontSize.focus();
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
  expect(screen.queryByRole('menu', { name: 'More query actions' })).toBeNull();
  await user.click(screen.getByRole('button', { name: 'More query actions' }));
  expect(
    screen.getByRole('menuitemcheckbox', { name: /Wrap lines/ }),
  ).toHaveAttribute('aria-checked', 'true');
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('menu', { name: 'More query actions' })).toBeNull();
});

it('opens the assistant from the AI button in the bar, with no separate corner button', async () => {
  const { user } = renderTab();
  expect(screen.queryByRole('button', { name: 'Assistant' })).toBeNull();
  const bar = within(screen.getByRole('toolbar', { name: 'Query actions' }));
  await user.click(bar.getByRole('button', { name: 'AI' }));
  expect(
    await screen.findByRole('complementary', { name: 'Query assistant' }),
  ).toBeVisible();
  await user.click(bar.getByRole('button', { name: 'AI' }));
  expect(
    screen.queryByRole('complementary', { name: 'Query assistant' }),
  ).toBeNull();
});

it('offers ⌘I inline editing from the More menu and the shortcut', async () => {
  const { user } = renderTab();
  const bar = within(screen.getByRole('toolbar', { name: 'Query actions' }));
  await user.click(bar.getByRole('button', { name: 'More query actions' }));
  expect(screen.getAllByRole('menuitem')[0]).toHaveTextContent(
    /Edit SQL with AI/,
  );
  await user.click(screen.getByRole('menuitem', { name: /Edit SQL with AI/ }));
  expect(editorText()).toMatch(/^-- inline prompt\n/);
  await user.click(screen.getByRole('textbox', { name: 'SQL editor' }));
  await user.keyboard('{Meta>}i{/Meta}');
  expect(editorText()).toMatch(/^-- inline prompt\n-- inline prompt\n/);
});
