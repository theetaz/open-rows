// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { shadowGrid } from './accessibility';
import { navigation, props } from './testFixture';
it('moves the painted range with keyboard focus and extends from a fixed Shift anchor', () => {
  const host = document.createElement('div');
  const input = props();
  input.onSelection = vi.fn();
  const shadow = shadowGrid(host, input, navigation());
  const grid = host.querySelector('[role=grid]');
  grid?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown' }));
  expect(input.selection.ranges).toEqual([{ r0: 3, r1: 3, c0: 2, c1: 2 }]);
  grid?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ArrowDown', shiftKey: true }),
  );
  grid?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ArrowDown', shiftKey: true }),
  );
  expect(input.selection.ranges).toEqual([{ r0: 3, r1: 5, c0: 2, c1: 2 }]);
  grid?.dispatchEvent(
    new KeyboardEvent('keydown', { key: ' ', ctrlKey: true }),
  );
  expect(input.selection.ranges[0]).toEqual({
    r0: 0,
    r1: 999999,
    c0: 2,
    c1: 2,
  });
  shadow.destroy();
});
it('jumps to the edges with Cmd/Ctrl+arrows and extends to an edge with Shift', () => {
  const host = document.createElement('div');
  const input = props();
  const shadow = shadowGrid(host, input, navigation());
  const grid = host.querySelector('[role=grid]');
  const press = (key: string, extra: KeyboardEventInit): void => {
    grid?.dispatchEvent(new KeyboardEvent('keydown', { key, ...extra }));
  };
  press('ArrowDown', { metaKey: true, shiftKey: true });
  expect(input.selection.ranges).toEqual([
    { r0: 2, r1: 999_999, c0: 2, c1: 2 },
  ]);
  press('ArrowRight', { ctrlKey: true });
  expect(input.selection).toMatchObject({ row: 999_999, column: 99 });
  press('ArrowUp', { metaKey: true });
  press('ArrowLeft', { metaKey: true });
  expect(input.selection.ranges).toEqual([{ r0: 0, r1: 0, c0: 0, c1: 0 }]);
  shadow.destroy();
});
it('selects rows without the number gutter, prevents editing its labels, and invokes editing on Enter', () => {
  const host = document.createElement('div');
  const input = props();
  input.rowHeaderColumn = 0;
  const onEdit = vi.fn();
  input.onEdit = onEdit;
  input.selection = { row: 0, column: 1, ranges: [] };
  const shadow = shadowGrid(host, input, navigation());
  const grid = host.querySelector('[role=grid]');
  grid?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft' }));
  expect(input.selection.column).toBe(1);
  grid?.dispatchEvent(
    new KeyboardEvent('keydown', { key: ' ', shiftKey: true }),
  );
  expect(input.selection.ranges[0]).toEqual({ r0: 0, r1: 0, c0: 1, c1: 99 });
  grid?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
  expect(onEdit).toHaveBeenCalledWith({ row: 0, column: 1 });
  shadow.destroy();
});
it('opens the keyboard context menu at the focused logical cell', () => {
  const host = document.createElement('div');
  const input = props();
  const onContextMenu = vi.fn();
  input.onContextMenu = onContextMenu;
  const shadow = shadowGrid(host, input, navigation());
  host.querySelector('[role=grid]')?.dispatchEvent(
    new KeyboardEvent('keydown', {
      key: 'F10',
      shiftKey: true,
      cancelable: true,
    }),
  );
  expect(onContextMenu).toHaveBeenCalledWith(
    expect.objectContaining({
      row: input.selection.row,
      column: input.selection.column,
      kind: 'cell',
    }),
  );
  shadow.destroy();
});
it('scrolls with Page Down, keeps Tab inside the grid and leaves it with Control-Tab', () => {
  const host = document.createElement('div');
  const after = document.createElement('button');
  document.body.append(host, after);
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockReturnValue([
    new DOMRect(),
  ] as unknown as DOMRectList);
  const input = props();
  const moves = { pageRows: () => 20, moved: vi.fn() };
  const shadow = shadowGrid(host, input, moves);
  const grid = host.querySelector<HTMLElement>('[role=grid]');
  const press = (key: string, extra: KeyboardEventInit = {}): boolean =>
    grid?.dispatchEvent(
      new KeyboardEvent('keydown', { key, cancelable: true, ...extra }),
    ) ?? false;
  expect(grid?.getAttribute('aria-description')).toMatch(/Control-Tab/);
  press('PageDown');
  expect(moves.moved).toHaveBeenLastCalledWith(
    expect.objectContaining({ row: 22, column: 2 }),
    20,
  );
  expect(press('Tab')).toBe(false);
  expect(input.selection).toMatchObject({ row: 22, column: 3 });
  grid?.focus();
  expect(press('Tab', { ctrlKey: true })).toBe(false);
  expect(document.activeElement).toBe(after);
  shadow.destroy();
  host.remove();
  after.remove();
});
