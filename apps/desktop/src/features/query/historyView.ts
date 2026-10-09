// SPDX-License-Identifier: Apache-2.0
import type { HistoryItemDto } from '@openrows/ipc';

/** When the entry started, in epoch milliseconds (the wire type allows null). */
const startOf = (item: HistoryItemDto): Date => new Date(item.startedAtMs ?? 0);

/** Entries of one local calendar day, newest first. */
type HistoryGroup = {
  key: string;
  label: string;
  items: readonly HistoryItemDto[];
};

/** Local calendar day as `YYYY-MM-DD`, the grouping key. */
function dayKey(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${String(date.getFullYear())}-${month}-${day}`;
}

/** The local day `offset` days from `now` (negative: earlier). */
function dayOffset(now: Date, offset: number): Date {
  return new Date(now.getFullYear(), now.getMonth(), now.getDate() + offset);
}

function labelOf(day: Date, now: Date): string {
  const key = dayKey(day);
  if (key === dayKey(now)) return 'Today';
  if (key === dayKey(dayOffset(now, -1))) return 'Yesterday';
  return day.toLocaleDateString(undefined, {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });
}

/**
 * Split newest-first entries into runs of one local day, labelled Today,
 * Yesterday or with the date. The backend already orders them newest first.
 */
export function groupByDay(
  items: readonly HistoryItemDto[],
  now: Date,
): HistoryGroup[] {
  const groups: { key: string; label: string; items: HistoryItemDto[] }[] = [];
  for (const item of items) {
    const day = startOf(item);
    const key = dayKey(day);
    const last = groups.at(-1);
    if (last?.key === key) last.items.push(item);
    else groups.push({ key, label: labelOf(day, now), items: [item] });
  }
  return groups;
}

/** The entry's facts in reading order: time, duration, rows and failure. */
export function summaryOf(item: HistoryItemDto): string[] {
  const parts = [
    startOf(item).toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
    }),
  ];
  if (item.durationMs !== null) parts.push(`${String(item.durationMs)} ms`);
  if (item.rows !== null)
    parts.push(
      `${item.rows.toLocaleString('en-US')} ${item.rows === 1 ? 'row' : 'rows'}`,
    );
  if (item.status === 'error') parts.push('Failed');
  if (item.status === 'cancelled') parts.push('Cancelled');
  return parts;
}
