// SPDX-License-Identifier: Apache-2.0
import type { EngineDto } from '@openrows/ipc';
import { displayWindow } from '../displayWindow';
import type { MenuEntry } from '../../shell/CascadeMenu';
import { COPY_AS, formatTable } from './formats';
import type { CopyFormat } from './formats';
import type { CopySettings } from './copySettings';
import { CopyError, readSelection } from './read';
import type { CopySource } from './read';
import { tableName } from './sql';
import { copyConflict, copySettings, delimitedOptions } from './copySettings';

/** Where copied SQL goes: the connection's dialect and the browsed table, if any. */
export type CopyTarget = {
  engine: EngineDto;
  /** The table a table tab browses; query results use `table_name`. */
  table: { schema: string; name: string } | null;
};

/** Placeholder INSERT target for ad-hoc results, as TablePlus writes it. */
const UNKNOWN_TABLE = { schema: '', name: 'table_name' };

/**
 * Copy the selected cells to the clipboard in `format`, with the copy
 * settings' header, NULL, quoting, delimiter, newline and decimal defaults
 * for TSV and CSV (HTML tables also go to the clipboard as HTML); `headers`
 * overrides the header setting (Copy with Headers). A palette command can
 * call this with the focused grid's selection.
 */
export async function copySelection(
  format: CopyFormat,
  source: CopySource,
  target: CopyTarget,
  headers?: boolean,
): Promise<void> {
  const settings = copySettings.get();
  const conflict = copyConflict(settings);
  if (conflict) throw new CopyError(conflict);
  const text = selectionText({ format, source, target, settings, headers });
  await writeClipboard(text, format === 'html');
}

/** Read asynchronous result windows, then format the selected cells. */
async function selectionText({
  format,
  source,
  target,
  settings,
  headers,
}: {
  format: CopyFormat;
  source: CopySource;
  target: CopyTarget;
  settings: CopySettings;
  headers: boolean | undefined;
}): Promise<string> {
  const table = await readSelection(source, displayWindow);
  const options =
    format === 'tsv' || format === 'csv' || format === 'html'
      ? delimitedOptions(format, settings, headers)
      : undefined;
  return formatTable(
    format,
    table,
    {
      engine: target.engine,
      table: tableName(target.engine, target.table ?? UNKNOWN_TABLE),
    },
    options,
  );
}

/** Start HTML writes during the user gesture; WebKit accepts promised representations. */
async function writeClipboard(
  text: Promise<string>,
  html: boolean,
): Promise<void> {
  if (!html || typeof ClipboardItem === 'undefined') {
    await navigator.clipboard.writeText(await text);
    return;
  }
  const item = new ClipboardItem({
    'text/html': text.then((value) => new Blob([value], { type: 'text/html' })),
    'text/plain': text.then(
      (value) => new Blob([value], { type: 'text/plain' }),
    ),
  });
  await navigator.clipboard.write([item]);
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
