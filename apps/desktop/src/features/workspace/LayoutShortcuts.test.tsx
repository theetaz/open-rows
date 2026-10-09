// SPDX-License-Identifier: Apache-2.0
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
  for (const number of [1, 2, 3, 4, 5, 6, 7, 8, 9])
    expect(shortcut({ ...event, key: String(number) })).toBe(number - 1);
  expect(shortcut({ ...event, key: '0' })).toBeUndefined();
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
  key('t');
  expect(screen.queryByRole('tab', { name: /SQL Query/ })).toBeNull();
  for (const [index, name] of [
    'orders-local',
    'analytics-prod',
    'events-dev',
  ].entries()) {
    key(String(index + 1));
    expect(
      screen.getByRole('button', { name: `Switch to ${name}` }),
    ).toHaveAttribute('aria-pressed', 'true');
  }
  key('4');
  expect(
    screen.getByRole('button', { name: 'Switch to events-dev' }),
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
it('auto-collapses at 1099 and reopens an overlay at 900 without shrinking content', async () => {
  const user = userEvent.setup();
  mount();
  expect(isCompact(1100)).toBe(false);
  expect(isCompact(1099)).toBe(true);
  vi.stubGlobal('innerWidth', 900);
  fireEvent(window, new Event('resize'));
  const inspector = (): HTMLElement =>
    screen.getByLabelText('Inspector', { selector: 'aside' });
  expect(inspector()).toHaveAttribute('data-open', 'false');
  expect(inspector()).toHaveAttribute('data-overlay', 'true');
  await user.click(
    screen.getByRole('button', { name: 'Toggle inspector (⌥⌘I)' }),
  );
  // A closed panel unmounts after its exit motion, so re-query each time.
  await waitFor(() => {
    expect(inspector()).toHaveAttribute('data-open', 'true');
  });
  vi.stubGlobal('innerWidth', 1100);
  fireEvent(window, new Event('resize'));
  expect(inspector()).toHaveAttribute('data-overlay', 'false');
  expect(inspector()).toHaveAttribute('data-open', 'true');
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
