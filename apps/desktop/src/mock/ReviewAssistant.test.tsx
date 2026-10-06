// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { Workspace } from './Workspace';
import { assistantSql } from './data/content';

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
});
function mount(): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  return userEvent.setup();
}
it('opens review, switches Changes/SQL, blocks workspace shortcuts and cancels', async () => {
  const user = mount();
  await user.click(screen.getByRole('button', { name: 'Review & Commit' }));
  const sheet = within(screen.getByRole('dialog', { name: 'Review & Commit' }));
  expect(sheet.getAllByRole('listitem')).toHaveLength(4);
  await user.click(sheet.getByRole('radio', { name: 'SQL' }));
  expect(sheet.getByText('COMMIT', { selector: 'span' })).toBeVisible();
  fireEvent.keyDown(window, { key: 't', code: 'KeyT', metaKey: true });
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  await user.click(sheet.getByRole('radio', { name: 'Changes' }));
  await user.click(sheet.getByRole('button', { name: 'Cancel' }));
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(screen.getByRole('button', { name: 'Review & Commit' })).toHaveFocus();
});
it('discards the mock edits and closes the review sheet', async () => {
  const user = mount();
  await user.click(screen.getByRole('button', { name: 'Review & Commit' }));
  await user.click(
    within(screen.getByRole('dialog')).getByRole('button', { name: 'Discard' }),
  );
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(screen.queryByRole('group', { name: 'Pending changes' })).toBeNull();
  expect(screen.queryByRole('row', { name: 'Deleted order 10427' })).toBeNull();
  expect(screen.getByRole('row', { name: 'Order 10427' })).toBeVisible();
  expect(
    screen.getByRole('grid').querySelector('[data-edited="true"]'),
  ).toBeNull();
});
it('discards directly from the capsule and cancels a sheet with Escape', async () => {
  const user = mount();
  await user.click(screen.getByRole('button', { name: 'Review & Commit' }));
  fireEvent(
    screen.getByRole('dialog'),
    new Event('cancel', { cancelable: true }),
  );
  expect(screen.queryByRole('dialog')).toBeNull();
  await user.click(screen.getByRole('button', { name: 'Discard' }));
  expect(screen.queryByRole('group', { name: 'Pending changes' })).toBeNull();
});
it('composes locally, copies SQL and opens the assistant proposal in a new tab', async () => {
  const user = mount();
  expect(screen.getByRole('button', { name: /Claude Code CLI/ })).toBeVisible();
  await user.type(
    screen.getByRole('textbox', { name: 'Ask the assistant' }),
    'Show revenue',
  );
  await user.click(screen.getByRole('button', { name: 'Send' }));
  expect(screen.getByText('Show revenue')).toBeVisible();
  expect(
    screen.getByRole('textbox', { name: 'Ask the assistant' }),
  ).toHaveValue('');
  const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
  await user.click(screen.getByRole('button', { name: 'Copy SQL' }));
  expect(write).toHaveBeenCalledWith(assistantSql);
  expect(screen.getByRole('button', { name: 'SQL copied' })).toBeVisible();
  await user.click(screen.getByRole('button', { name: 'Open in tab' }));
  expect(screen.getByRole('textbox', { name: 'SQL editor' })).toHaveValue(
    assistantSql,
  );
  await user.click(
    within(screen.getByRole('complementary', { name: 'Inspector' })).getByRole(
      'button',
      { name: 'Run read-only' },
    ),
  );
  expect(screen.getByRole('tab', { name: /SQL Query 2/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
});
it('reports blocked clipboard access inline without an alert', async () => {
  const user = mount();
  vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(
    new Error('Blocked'),
  );
  await user.click(screen.getByRole('button', { name: 'Copy SQL' }));
  expect(
    screen.getByRole('button', { name: 'Copy unavailable' }),
  ).toBeVisible();
  expect(screen.queryByRole('alert')).toBeNull();
});
