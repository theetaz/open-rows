// SPDX-License-Identifier: Apache-2.0
import type { DatabaseDto } from '@openrows/ipc';

/** Databases matching `query` (case-insensitive substring); system ones only on request. */
export function visibleDatabases(
  databases: readonly DatabaseDto[],
  query: string,
  system: boolean,
): DatabaseDto[] {
  const needle = query.trim().toLowerCase();
  return databases.filter(
    (database) =>
      (system || !database.system) &&
      database.name.toLowerCase().includes(needle),
  );
}

/** Index after ↑/↓ in the search field, wrapping; `undefined` for other keys. */
export function listIndex(
  key: string,
  index: number,
  length: number,
): number | undefined {
  if (!length) return undefined;
  if (key === 'ArrowDown') return (index + 1) % length;
  if (key === 'ArrowUp') return (index - 1 + length) % length;
  return undefined;
}
