// SPDX-License-Identifier: Apache-2.0
import type { ExecuteRequest } from '@openrows/ipc';

/** Read access to the editor that owns a tab's SQL. */
export type EditorHandle = {
  text: () => string;
  /** Cursor as a UTF-16 offset (CodeMirror position). */
  cursor: () => number;
  selection: () => string;
  focus: () => void;
};

/** Run current (⌘↵, or the selection when text is selected) or all (⇧⌘↵). */
export type RunScope = 'current' | 'all';

/** The `query_execute` request for a run; `null` when there is nothing to run. */
export function runRequest(
  editor: EditorHandle,
  scope: RunScope,
  sessionId: string,
): ExecuteRequest | null {
  const selection = editor.selection();
  const base = { sessionId, cursor: null, confirmToken: null };
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
