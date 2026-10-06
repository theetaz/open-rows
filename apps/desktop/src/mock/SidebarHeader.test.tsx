// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import axe from 'axe-core';
import { expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { SidebarHeader } from './SidebarHeader';
import { connections } from './data/connections';
const connection = connections[0];
if (!connection) throw new Error('Missing mock workspace');
const props = {
  connection,
  database: 'orders_archive',
  opened: ['orders', 'orders_archive'],
  onDatabase: vi.fn(),
  onCloseDatabase: vi.fn(),
  onSwitcher: vi.fn(),
};
function mount(): ReturnType<typeof userEvent.setup> {
  render(
    <ThemeProvider>
      <SidebarHeader {...props} />
    </ThemeProvider>,
  );
  return userEvent.setup();
}
it('shows workspace identity and local connection menu pressed states', async () => {
  const user = mount();
  expect(screen.getByText('orders-staging')).toBeVisible();
  expect(screen.getByText('staging')).toBeVisible();
  const opener = screen.getByRole('button', { name: 'Workspace menu' });
  await user.click(opener);
  expect(opener).toHaveAttribute('aria-pressed', 'true');
  for (const name of ['Connection settings…', 'Duplicate', 'Disconnect']) {
    await user.click(screen.getByRole('menuitemcheckbox', { name }));
    expect(screen.getByRole('menuitemcheckbox', { name })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  }
  expect(props.onDatabase).not.toHaveBeenCalled();
  await user.keyboard('{Escape}');
  expect(opener).toHaveFocus();
  expect(opener).toHaveAttribute('aria-pressed', 'false');
});
it('lists opened databases with active check and keyboard switch, close and focus return', async () => {
  const user = mount();
  const opener = screen.getByRole('button', {
    name: 'Opened databases (orders_archive)',
  });
  await user.click(opener);
  const active = screen.getByRole('menuitemradio', {
    name: 'Activate database orders_archive',
  });
  expect(active).toHaveFocus();
  expect(active).toHaveAttribute('aria-checked', 'true');
  expect(active).toHaveTextContent('✓');
  expect(screen.getAllByRole('menuitemradio')).toHaveLength(2);
  await user.keyboard('{Home}{Enter}');
  expect(props.onDatabase).toHaveBeenCalledWith('orders');
  expect(opener).toHaveFocus();
  expect(screen.queryByRole('menu')).toBeNull();
  await user.click(opener);
  await user.keyboard('{ArrowDown}{Enter}');
  expect(props.onCloseDatabase).toHaveBeenCalledWith('orders_archive');
  expect(opener).toHaveFocus();
});
it('wraps navigation, ignores composition, closes with Tab/Escape/outside pointer and opens switcher', async () => {
  const user = mount();
  const opener = screen.getByRole('button', {
    name: 'Opened databases (orders_archive)',
  });
  await user.click(opener);
  await user.keyboard('{End}{ArrowDown}');
  expect(
    screen.getByRole('menuitemradio', { name: 'Activate database orders' }),
  ).toHaveFocus();
  fireEvent.keyDown(document.activeElement ?? document.body, {
    key: 'ArrowDown',
    isComposing: true,
  });
  expect(
    screen.getByRole('menuitemradio', { name: 'Activate database orders' }),
  ).toHaveFocus();
  await user.keyboard('{ArrowUp}{Enter}');
  expect(props.onSwitcher).toHaveBeenCalledOnce();
  expect(opener).toHaveFocus();
  for (const key of ['{Tab}', '{Escape}']) {
    await user.click(opener);
    await user.keyboard(key);
    expect(opener).toHaveFocus();
    expect(screen.queryByRole('menu')).toBeNull();
  }
  await user.click(opener);
  fireEvent.pointerDown(document.body);
  expect(screen.queryByRole('menu')).toBeNull();
  expect(opener).toHaveFocus();
});
it('protects the last database and leaves close reachable only when enabled', async () => {
  const user = userEvent.setup();
  render(<SidebarHeader {...props} database="orders" opened={['orders']} />);
  await user.click(
    screen.getByRole('button', { name: 'Opened databases (orders)' }),
  );
  expect(
    screen.getByRole('menuitem', { name: 'Close database orders' }),
  ).toBeDisabled();
  await user.keyboard('{ArrowDown}');
  expect(screen.getByRole('menuitem', { name: /Open database/ })).toHaveFocus();
});
it.each(['light', 'dark'] as const)(
  'axe: header and both menus clean in %s',
  async (theme) => {
    localStorage.setItem('openrows.theme', theme);
    const user = mount();
    for (const name of [
      'Workspace menu',
      'Opened databases (orders_archive)',
    ]) {
      await user.click(screen.getByRole('button', { name }));
      const result = await axe.run(document.body, {
        rules: {
          'color-contrast': { enabled: false },
          region: { enabled: false },
        },
      });
      expect(result.violations).toEqual([]);
      await user.keyboard('{Escape}');
    }
  },
);
