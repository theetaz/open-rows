// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it } from 'vitest';
import type { HistoryItemDto } from '@openrows/ipc';
import { groupByDay, summaryOf } from './historyView';

const at = (month: number, day: number, hour: number): number =>
  new Date(2026, month - 1, day, hour).getTime();

function entry(
  id: number,
  startedAtMs: number,
  extra: Partial<HistoryItemDto> = {},
): HistoryItemDto {
  return {
    id,
    profileId: 'p1',
    database: 'shop',
    statement: `SELECT ${String(id)}`,
    startedAtMs,
    durationMs: null,
    status: 'ok',
    rows: null,
    errorMessage: null,
    ...extra,
  };
}

describe('groupByDay', () => {
  it('labels today and yesterday and keeps the newest-first order inside a day', () => {
    const now = new Date(2026, 9, 9, 15);
    const groups = groupByDay(
      [
        entry(3, at(10, 9, 14)),
        entry(2, at(10, 9, 8)),
        entry(1, at(10, 8, 20)),
        entry(0, at(9, 30, 10)),
      ],
      now,
    );
    expect(groups.map((group) => group.label)).toEqual([
      'Today',
      'Yesterday',
      new Date(2026, 8, 30).toLocaleDateString(undefined, {
        weekday: 'long',
        day: 'numeric',
        month: 'long',
        year: 'numeric',
      }),
    ]);
    expect(groups[0]?.items.map((item) => item.id)).toEqual([3, 2]);
  });

  it('is empty for an empty history', () => {
    expect(groupByDay([], new Date())).toEqual([]);
  });
});

describe('summaryOf', () => {
  it('names the duration, rows and failure of an entry', () => {
    const parts = summaryOf(
      entry(1, at(10, 9, 9), {
        durationMs: 3,
        rows: 1,
        status: 'error',
        errorMessage: 'boom',
      }),
    );
    expect(parts.slice(1)).toEqual(['3 ms', '1 row', 'Failed']);
  });

  it('pluralises rows and marks cancelled runs', () => {
    const parts = summaryOf(
      entry(1, at(10, 9, 9), { rows: 12, status: 'cancelled' }),
    );
    expect(parts.slice(1)).toEqual(['12 rows', 'Cancelled']);
  });
});
