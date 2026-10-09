// SPDX-License-Identifier: Apache-2.0
import type { ExecuteRequest } from '@openrows/ipc';
import type { FormatKind, FormatOptions } from '@openrows/editor';

/** Read access to the editor that owns a tab's SQL. */
export type EditorHandle = {
  text: () => string;
  /** Cursor as a UTF-16 offset (CodeMirror position). */
  cursor: () => number;
  selection: () => string;
  focus: () => void;
  /** Beautify or uglify the selection, current statement or document. */
  format: (kind: FormatKind, options: FormatOptions) => Promise<void>;
};

/** Run current (⌘↵, or the selection when text is selected) or all (⇧⌘↵). */
export type RunScope = 'current' | 'all';

/**
 * The `query_execute` request for a run; `null` when there is nothing to run.
 * `maxRows` caps each result set (the bottom bar's limit; `null` is none).
 */
export function runRequest(
  editor: EditorHandle,
  scope: RunScope,
  sessionId: string,
  maxRows: number | null,
): ExecuteRequest | null {
  const selection = editor.selection();
  const base = { sessionId, cursor: null, confirmToken: null, maxRows };
  if (scope === 'current' && selection.trim())
    return { ...base, sql: selection, mode: 'selection' };
  const sql = editor.text();
  if (!sql.trim()) return null;
  return scope === 'all'
    ? { ...base, sql, mode: 'all' }
    : { ...base, sql, mode: 'current', cursor: editor.cursor() };
}

/** Run shortcuts anywhere in a query tab (the editor handles its own first). */
export function runScopeOf(event: {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  defaultPrevented: boolean;
}): RunScope | null {
  if (event.defaultPrevented || event.key !== 'Enter' || event.altKey)
    return null;
  if (!event.metaKey && !event.ctrlKey) return null;
  return event.shiftKey ? 'all' : 'current';
}

/** ⌘I beautifies and ⇧⌘I uglifies anywhere in a query tab. */
export function formatKindOf(event: {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  defaultPrevented: boolean;
}): FormatKind | null {
  if (event.defaultPrevented || event.altKey) return null;
  if (event.key.toLowerCase() !== 'i' || (!event.metaKey && !event.ctrlKey))
    return null;
  return event.shiftKey ? 'uglify' : 'beautify';
}

/** Tab file commands: ⌘S save, ⇧⌘S save as, ⌘O open. */
export type FileAction = 'save' | 'saveAs' | 'open';

/** ⌘S, ⇧⌘S and ⌘O anywhere in a query tab. */
export function fileActionOf(event: {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  defaultPrevented: boolean;
}): FileAction | null {
  if (event.defaultPrevented || event.altKey || event.ctrlKey) return null;
  if (!event.metaKey) return null;
  const key = event.key.toLowerCase();
  if (key === 's') return event.shiftKey ? 'saveAs' : 'save';
  return key === 'o' && !event.shiftKey ? 'open' : null;
}
