// SPDX-License-Identifier: Apache-2.0
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { shadowGrid } from './accessibility';
import { ANNOUNCE_INTERVAL_MS, describeSelection } from './announce';
import { navigation, props } from './testFixture';
import type { GridProps } from './model';

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

function mounted(input: GridProps = props()): {
  host: HTMLElement;
  press: (key: string, extra?: KeyboardEventInit) => void;
  status: () => string | null | undefined;
  destroy: () => void;
} {
  const host = document.createElement('div');
  const shadow = shadowGrid(host, input, navigation());
  const grid = host.querySelector('[role=grid]');
  return {
    host,
    press: (key, extra = {}) =>
      grid?.dispatchEvent(new KeyboardEvent('keydown', { key, ...extra })),
    status: () => host.querySelector('[role=status]')?.textContent,
    destroy: shadow.destroy,
  };
}

it('describes one cell, ranges, whole rows and whole columns concisely', () => {
  const input = props();
  const at = (ranges: GridProps['selection']['ranges']): string =>
    describeSelection(input, { row: 11, column: 2, ranges });
  expect(at([])).toBe('Row 12, column_2: Loading');
  expect(at([{ r0: 11, r1: 11, c0: 2, c1: 2 }])).toBe(
    'Row 12, column_2: Loading',
  );
  expect(at([{ r0: 11, r1: 13, c0: 2, c1: 3 }])).toBe(
    'Selected 3 rows by 2 columns',
  );
  expect(at([{ r0: 11, r1: 11, c0: 0, c1: 99 }])).toBe('Selected row 12');
  expect(at([{ r0: 0, r1: 999_999, c0: 2, c1: 2 }])).toBe(
    'Selected column column_2',
  );
  expect(at([{ r0: 0, r1: 999_998, c0: 2, c1: 3 }])).toBe(
    'Selected 999,999 rows by 2 columns',
  );
  expect(
    at([
      { r0: 1, r1: 1, c0: 1, c1: 1 },
      { r0: 4, r1: 5, c0: 1, c1: 1 },
    ]),
  ).toBe('Selected 2 ranges');
});

it('throttles keyboard moves to one announcement of the latest selection', () => {
  const grid = mounted();
  expect(grid.status()).toBe('');
  grid.press('ArrowDown');
  grid.press('ArrowDown');
  grid.press('ArrowRight');
  expect(grid.status()).toBe('');
  vi.advanceTimersByTime(ANNOUNCE_INTERVAL_MS);
  expect(grid.status()).toBe('Row 5, column_3: Loading');
  grid.press('ArrowDown', { shiftKey: true });
  grid.press('ArrowRight', { shiftKey: true });
  vi.advanceTimersByTime(ANNOUNCE_INTERVAL_MS);
  expect(grid.status()).toBe('Selected 2 rows by 2 columns');
  grid.destroy();
});

it('re-announces a focused value once its tile arrives, but not unrelated tiles', async () => {
  const input = props();
  const grid = mounted(input);
  grid.press('ArrowDown');
  vi.advanceTimersByTime(ANNOUNCE_INTERVAL_MS);
  expect(grid.status()).toBe('Row 4, column_2: Loading');
  await input.source.window({ r0: 0, r1: 20, c0: 0, c1: 5 }, 1);
  vi.advanceTimersByTime(ANNOUNCE_INTERVAL_MS);
  expect(grid.status()).toBe('Row 4, column_2: 3');
  await input.source.window({ r0: 600, r1: 620, c0: 0, c1: 5 }, 1);
  vi.advanceTimersByTime(ANNOUNCE_INTERVAL_MS);
  expect(grid.status()).toBe('Row 4, column_2: 3');
  grid.destroy();
});

it('announces edit and staging messages at once and drops a pending move', () => {
  const host = document.createElement('div');
  const shadow = shadowGrid(host, props(), navigation());
  host
    .querySelector('[role=grid]')
    ?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp' }));
  shadow.announce('Editing column_2');
  vi.advanceTimersByTime(ANNOUNCE_INTERVAL_MS);
  const status = host.querySelector('[role=status]');
  expect(status?.textContent).toBe('Editing column_2');
  shadow.announce('Editing column_2');
  expect(status?.textContent).toBe('Editing column_2 ');
  shadow.destroy();
});

it('exposes the column header and neighbouring rows around the active cell', async () => {
  const input = props();
  input.rowHeaderColumn = 0;
  input.selection = {
    row: 1,
    column: 3,
    ranges: [{ r0: 1, r1: 2, c0: 3, c1: 3 }],
  };
  await input.source.window({ r0: 0, r1: 20, c0: 0, c1: 5 }, 1);
  const grid = mounted(input);
  const rows = [...grid.host.querySelectorAll<HTMLElement>('[role=row]')];
  const visible = rows.filter((row) => !row.hidden);
  expect(visible.map((row) => row.getAttribute('aria-rowindex'))).toEqual([
    '1',
    '2',
    '3',
    '4',
  ]);
  const header = grid.host.querySelector('[role=columnheader]');
  expect(header?.getAttribute('aria-colindex')).toBe('4');
  expect(header?.textContent).toBe('column_3');
  const active = grid.host.querySelector(
    `#${grid.host.querySelector('[role=grid]')?.getAttribute('aria-activedescendant') ?? ''}`,
  );
  expect(active?.parentElement?.getAttribute('aria-rowindex')).toBe('3');
  expect(active?.getAttribute('aria-selected')).toBe('true');
  const rowHeader = active?.parentElement?.querySelector('[role=rowheader]');
  expect(rowHeader?.getAttribute('aria-colindex')).toBe('1');
  expect(rowHeader?.textContent).toBe('1');
  expect(visible[3]?.querySelector('[role=gridcell]')?.textContent).toBe('2');
  grid.press('End', { metaKey: true });
  expect(
    rows
      .filter((row) => !row.hidden)
      .map((row) => row.getAttribute('aria-rowindex')),
  ).toEqual(['1', '1000000', '1000001']);
  grid.destroy();
});
