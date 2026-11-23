// SPDX-License-Identifier: Apache-2.0
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { Menu } from './Menu';
import type { MenuEntry } from './menuModel';
import { aimsAtSubmenu } from './safeTriangle';
import { menuKey } from './menuKeys';
import { hover, openSubmenu, step } from './menuModel';

const open = vi.fn();
const drop = vi.fn();
const json = vi.fn();
const entries: readonly MenuEntry[] = [
  { kind: 'header', label: 'Table' },
  { kind: 'item', label: 'Open', run: open, shortcut: '↩' },
  { kind: 'item', label: 'Rename', run: vi.fn(), disabled: true },
  {
    kind: 'submenu',
    label: 'Copy As',
    entries: [
      { kind: 'item', label: 'CSV', run: vi.fn() },
      { kind: 'item', label: 'JSON', run: json },
    ],
  },
  { kind: 'separator' },
  { kind: 'item', label: 'Drop Table…', run: drop, danger: true },
];

afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});

function mount(keyboard = false): () => void {
  const onClose = vi.fn();
  render(
    <Menu
      label="Table"
      at={{ x: 40, y: 60 }}
      entries={entries}
      onClose={onClose}
      keyboard={keyboard}
    />,
  );
  return onClose;
}

it('skips headers, separators and disabled rows with wrapping arrows', () => {
  expect(step(entries, null, 1)).toBe(1);
  expect(step(entries, 1, 1)).toBe(3);
  expect(step(entries, 5, 1)).toBe(1);
  expect(step(entries, 1, -1)).toBe(5);
});

it('opens a submenu on → at its first row and goes back on ←', () => {
  vi.useFakeTimers();
  mount(true);
  const menu = screen.getByRole('menu', { name: 'Table' });
  expect(screen.getByRole('menuitem', { name: /Open/ })).toHaveFocus();
  fireEvent.keyDown(menu, { key: 'ArrowDown' });
  fireEvent.keyDown(menu, { key: 'ArrowRight' });
  expect(screen.getByRole('menu', { name: 'Copy As' })).toBeInTheDocument();
  expect(screen.getByRole('menuitem', { name: 'CSV' })).toHaveFocus();
  fireEvent.keyDown(menu, { key: 'ArrowLeft' });
  expect(screen.queryByRole('menu', { name: 'Copy As' })).toBeNull();
  expect(screen.getByRole('menuitem', { name: /Copy As/ })).toHaveFocus();
});

it('blinks, fades, closes and then runs the chosen command', () => {
  vi.useFakeTimers();
  const onClose = mount(true);
  const row = screen.getByRole('menuitem', { name: /Open/ });
  fireEvent.keyDown(row, { key: 'Enter' });
  expect(row).toHaveAttribute('data-chosen', 'true');
  act(() => {
    vi.advanceTimersByTime(120);
  });
  expect(screen.getByRole('menu', { name: 'Table' })).toHaveAttribute(
    'data-state',
    'closing',
  );
  expect(open).not.toHaveBeenCalled();
  act(() => {
    vi.advanceTimersByTime(90);
  });
  expect(onClose).toHaveBeenCalledOnce();
  expect(open).toHaveBeenCalledOnce();
});

it('ignores disabled rows and closes on Escape', () => {
  vi.useFakeTimers();
  const onClose = mount();
  fireEvent.click(screen.getByRole('menuitem', { name: 'Rename' }));
  const menu = screen.getByRole('menu', { name: 'Table' });
  fireEvent.keyDown(menu, { key: 'Escape' });
  act(() => {
    vi.advanceTimersByTime(90);
  });
  expect(onClose).toHaveBeenCalledOnce();
});

it('closes at once on a click outside', () => {
  const onClose = mount();
  fireEvent.pointerDown(document.body);
  expect(onClose).toHaveBeenCalledOnce();
});

it('finds rows by type-ahead and marks destructive rows', () => {
  const state = { open: [], active: [null] };
  const result = menuKey('d', state, entries, 'd');
  expect(result).toEqual({
    kind: 'state',
    state: { open: [], active: [5] },
  });
  mount();
  expect(screen.getByRole('menuitem', { name: /Drop Table/ })).toHaveAttribute(
    'data-danger',
    'true',
  );
});

it('keeps an open submenu while hovering its parent row', () => {
  const opened = openSubmenu({ open: [], active: [null] }, 0, 3, null);
  expect(hover(opened, 0, 3)).toEqual(opened);
  expect(hover(opened, 0, 1)).toEqual({ open: [], active: [1] });
});

it('aims at a submenu inside the safe triangle only', () => {
  const submenu = { left: 200, right: 400, top: 100, bottom: 300 };
  const parent = { left: 0 };
  const from = { x: 150, y: 120 };
  expect(aimsAtSubmenu(from, { x: 180, y: 140 }, submenu, parent)).toBe(true);
  expect(aimsAtSubmenu(from, { x: 150, y: 160 }, submenu, parent)).toBe(false);
});
