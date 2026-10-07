// SPDX-License-Identifier: Apache-2.0
import { expect, it, vi } from 'vitest';
import { shadowGrid } from './accessibility';
import { props } from './testFixture';
it('moves the painted range with keyboard focus and extends from a fixed Shift anchor', () => {
  const host = document.createElement('div');
  const input = props();
  input.onSelection = vi.fn();
  const shadow = shadowGrid(host, input);
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
it('selects rows without the number gutter, prevents editing its labels, and invokes editing on Enter', () => {
  const host = document.createElement('div');
  const input = props();
  input.rowHeaderColumn = 0;
  const onEdit = vi.fn();
  input.onEdit = onEdit;
  input.selection = { row: 0, column: 1, ranges: [] };
  const shadow = shadowGrid(host, input);
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
  const shadow = shadowGrid(host, input);
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
