// SPDX-License-Identifier: Apache-2.0
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { Workspace } from './Workspace';
import { assistantSql } from './data/content';

vi.mock('@openrows/ipc', async () =>
  (await import('../test/ipcStub')).ipcStub(),
);

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
});
async function mount(): Promise<ReturnType<typeof userEvent.setup>> {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
  const user = userEvent.setup();
  await user.click(
    await screen.findByRole('button', { name: 'Switch to orders-local' }),
  );
  const inspector = within(
    screen.getByRole('complementary', { name: 'Inspector' }),
  );
  await user.click(inspector.getByRole('radio', { name: 'Assistant' }));
  return user;
}
it('composes locally, copies SQL and opens the assistant proposal in a new tab', async () => {
  const user = await mount();
  expect(
    await screen.findByRole('button', { name: /Claude Code CLI/ }),
  ).toBeVisible();
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
  const editor = await screen.findByRole('textbox', { name: 'SQL editor' });
  await waitFor(() => {
    expect(editor).toHaveTextContent('GROUP BY currency');
  });
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
  const user = await mount();
  vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(
    new Error('Blocked'),
  );
  await user.click(await screen.findByRole('button', { name: 'Copy SQL' }));
  expect(
    screen.getByRole('button', { name: 'Copy unavailable' }),
  ).toBeVisible();
  expect(screen.queryByRole('alert')).toBeNull();
});
