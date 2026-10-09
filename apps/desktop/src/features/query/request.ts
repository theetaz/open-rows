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
  /** Beautify or uglify the selection, else the whole document. */
  format: (kind: FormatKind, options: FormatOptions) => Promise<void>;
  /** Replace the whole text (Reload of a file changed on disk). */
  replace: (text: string) => void;
  /** Insert a snippet template over the selection. */
  insertSnippet: (body: string) => void;
  /** Where the text Run Current executes starts (selection or statement). */
  runStart: () => number;
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

/** Tab file commands: ⌘S save, ⇧⌘S save as, ⌘O open. */
export type FileAction = 'save' | 'saveAs' | 'open';
