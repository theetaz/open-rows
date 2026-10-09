// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';
import { displayWindow } from '../displayWindow';
import type { MenuEntry } from '../../shell/CascadeMenu';
import { COPY_AS, formatTable } from './formats';
import type { CopyFormat } from './formats';
import { readSelection } from './read';
import type { CopySource } from './read';
import { tableName } from './sql';

/** Where copied SQL goes: the connection's dialect and the browsed table, if any. */
export type CopyTarget = {
  engine: EngineDto;
  /** The table a table tab browses; query results use `table_name`. */
  table: { schema: string; name: string } | null;
};

/** Placeholder INSERT target for ad-hoc results, as TablePlus writes it. */
const UNKNOWN_TABLE = { schema: '', name: 'table_name' };

/**
 * Copy the selected cells to the clipboard in `format`. A palette command
 * can call this with the focused grid's selection.
 */
export async function copySelection(
  format: CopyFormat,
  source: CopySource,
  target: CopyTarget,
): Promise<void> {
  const table = await readSelection(source, displayWindow);
  const text = formatTable(format, table, {
    engine: target.engine,
    table: tableName(target.engine, target.table ?? UNKNOWN_TABLE),
  });
  await navigator.clipboard.writeText(text);
}

/** The `Copy as ▸` submenu of a grid context menu. */
export function copyAsMenu(run: (format: CopyFormat) => void): MenuEntry {
  return {
    kind: 'submenu',
    label: 'Copy as',
    entries: COPY_AS.map(({ format, label }) => ({
      kind: 'item',
      label,
      run: () => {
        run(format);
      },
    })),
  };
}

/** A failure message for a copy that did not reach the clipboard. */
export function copyFailure(error: unknown): string {
  if (error instanceof Error && error.name === 'CopyError')
    return error.message;
  return `Could not copy: ${error instanceof Error ? error.message : String(error)}`;
}
