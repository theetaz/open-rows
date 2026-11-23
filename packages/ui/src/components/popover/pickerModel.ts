// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from 'react';

/** One row of a picker. */
export type PickerItem = {
  id: string;
  name: string;
  /** Leading icon or engine tile. */
  lead?: ReactNode;
  /** Second line (host, path); makes the row 40 px. */
  sub?: string;
  /** Right-aligned meta: size, counts, an environment tag. */
  meta?: ReactNode;
  /** A status dot after the meta. */
  status?: 'on' | 'idle' | 'warn' | 'off';
  /** The current choice: checked, and highlighted when the picker opens. */
  current?: boolean;
};
/** A titled group of rows (Recent, then all). */
export type PickerSection = { head?: string; items: readonly PickerItem[] };
/** A visible row with its section heading, if it starts a section. */
export type PickerRow = { item: PickerItem; head?: string };

/** Rows rendered at most; the search narrows longer lists. */
const PICKER_LIMIT = 200;

/** Case-insensitive substring filter over every section, headings kept. */
export function pickerRows(
  sections: readonly PickerSection[],
  query: string,
): { rows: PickerRow[]; total: number } {
  const wanted = query.trim().toLowerCase();
  const rows: PickerRow[] = [];
  let total = 0;
  for (const section of sections) {
    const items = wanted
      ? section.items.filter((item) => item.name.toLowerCase().includes(wanted))
      : section.items;
    total += items.length;
    for (const [position, item] of items.entries()) {
      if (rows.length >= PICKER_LIMIT) break;
      rows.push(
        position === 0 && section.head
          ? { item, head: section.head }
          : { item },
      );
    }
  }
  return { rows, total };
}

/** Where the highlight starts: the first match, or the current item. */
export function initialIndex(
  rows: readonly PickerRow[],
  query: string,
): number {
  if (query.trim() || !rows.length) return 0;
  return Math.max(
    0,
    rows.findIndex((row) => row.item.current),
  );
}

/** Split `name` around the first match of `query`, for a bold highlight. */
export function splitMatch(
  name: string,
  query: string,
): [string, string, string] {
  const wanted = query.trim();
  const at = wanted ? name.toLowerCase().indexOf(wanted.toLowerCase()) : -1;
  if (at < 0) return [name, '', ''];
  return [
    name.slice(0, at),
    name.slice(at, at + wanted.length),
    name.slice(at + wanted.length),
  ];
}
