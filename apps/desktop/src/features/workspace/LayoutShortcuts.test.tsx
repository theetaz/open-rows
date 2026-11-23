// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { ThemeProvider } from '@openrows/ui';
import { Workspace } from './Workspace';
import { isCompact } from './layout';
import { shortcut } from './shortcuts';

vi.mock('@openrows/ipc', async () =>
  (await import('../../test/ipcStub')).ipcStub(),
);

beforeEach(() => {
  vi.stubGlobal('innerWidth', 1280);
});
function mount(): void {
  render(
    <ThemeProvider>
      <Workspace onGallery={vi.fn()} />
    </ThemeProvider>,
  );
}
function key(key: string, modifiers: Partial<KeyboardEvent> = {}): void {
  fireEvent.keyDown(window, {
    key,
    code: `Key${key.toUpperCase()}`,
    metaKey: true,
    ...modifiers,
  });
}
it('handles the exact shortcut modifiers and ignores composing or shifted input', () => {
  const event = {
    key: 't',
    code: 'KeyT',
    metaKey: true,
    altKey: false,
    ctrlKey: false,
    shiftKey: false,
    isComposing: false,
  };
  expect(shortcut(event)).toBe('new-tab');
  expect(shortcut({ ...event, key: 'w' })).toBe('close-tab');
  expect(shortcut({ ...event, key: 's', code: 'KeyS', ctrlKey: true })).toBe(
    'sidebar',
  );
  expect(shortcut({ ...event, key: 'Dead', code: 'KeyI', altKey: true })).toBe(
    'inspector',
  );
  for (const override of [
    { metaKey: false },
    { shiftKey: true },
    { isComposing: true },
    { altKey: true },
    { ctrlKey: true },
  ])
    expect(shortcut({ ...event, ...override })).toBeUndefined();
  for (const number of [1, 2, 3, 4, 5, 6, 7, 8, 9]) {
    const digit = { key: String(number), code: `Digit${String(number)}` };
    expect(shortcut({ ...event, ...digit })).toBe(
      `select-tab-${String(number)}`,
    );
    // ⌥ types another character; the physical key still names the connection.
    expect(
      shortcut({ ...event, key: '¡', code: digit.code, altKey: true }),
    ).toBe(number - 1);
  }
  expect(shortcut({ ...event, key: '0' })).toBeUndefined();
  expect(
    shortcut({ ...event, key: 'Tab', metaKey: false, ctrlKey: true }),
  ).toBe('next-tab');
  expect(
    shortcut({
      ...event,
      key: 'Tab',
      metaKey: false,
      ctrlKey: true,
      shiftKey: true,
    }),
  ).toBe('previous-tab');
});
it('dispatches panel, tab and saved-connection shortcuts', async () => {
  mount();
  await screen.findByRole('button', { name: 'Switch to orders-local' });
  key('s', { ctrlKey: true });
  expect(
    screen.getByRole('button', { name: 'Toggle sidebar (⌃⌘S)' }),
  ).toHaveAttribute('aria-pressed', 'false');
  key('i', { altKey: true });
  expect(
    screen.getByRole('button', { name: 'Toggle inspector (⌥⌘I)' }),
  ).toHaveAttribute('aria-pressed', 'false');
  // The connection list lives in the sidebar: show it again to read it.
  key('s', { ctrlKey: true });
  expect(
    screen.getByRole('button', { name: 'Toggle sidebar (⌃⌘S)' }),
  ).toHaveAttribute('aria-pressed', 'true');
  key('t');
  expect(screen.queryByRole('tab', { name: /SQL Query/ })).toBeNull();
  for (const [index, name] of [
    'orders-local',
    'analytics-prod',
    'events-dev',
  ].entries()) {
    key(String(index + 1), { altKey: true });
    // Choosing a connection folds the list behind the sidebar pop-up.
    expect(
      screen.getByRole('button', { name: `Switch to ${name}`, hidden: true }),
    ).toHaveAttribute('aria-pressed', 'true');
  }
  key('4', { altKey: true });
  expect(
    screen.getByRole('button', { name: 'Switch to events-dev', hidden: true }),
  ).toHaveAttribute('aria-pressed', 'true');
  key('t');
  expect(screen.getByRole('tab', { name: /SQL Query 1/ })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  key('w');
  expect(screen.queryByRole('tab', { name: /SQL Query 1/ })).toBeNull();
  await screen.findAllByText('Connected · PostgreSQL 17.2');
});
it('auto-collapses at 1099, reopens at 900 on request and shows no empty inspector', async () => {
  const user = userEvent.setup();
  mount();
  expect(isCompact(1100)).toBe(false);
  expect(isCompact(1099)).toBe(true);
  vi.stubGlobal('innerWidth', 900);
  fireEvent(window, new Event('resize'));
  const toggle = screen.getByRole('button', {
    name: 'Toggle inspector (⌥⌘I)',
  });
  expect(toggle).toHaveAttribute('aria-pressed', 'false');
  await user.click(toggle);
  expect(toggle).toHaveAttribute('aria-pressed', 'true');
  vi.stubGlobal('innerWidth', 1100);
  fireEvent(window, new Event('resize'));
  expect(toggle).toHaveAttribute('aria-pressed', 'true');
  // Nothing is selected, so the inspector takes no room at all.
  expect(
    screen.queryByLabelText('Inspector', { selector: 'aside' }),
  ).toBeNull();
});
it('opens and closes both panels with toolbar controls', async () => {
  const user = userEvent.setup();
  mount();
  for (const name of ['Toggle sidebar (⌃⌘S)', 'Toggle inspector (⌥⌘I)']) {
    const button = screen.getByRole('button', { name });
    await user.click(button);
    expect(button).toHaveAttribute('aria-pressed', 'false');
    await user.click(button);
    expect(button).toHaveAttribute('aria-pressed', 'true');
  }
});
