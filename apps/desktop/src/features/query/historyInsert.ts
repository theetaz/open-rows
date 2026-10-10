// SPDX-License-Identifier: Apache-2.0
import { useEffect } from 'react';
import type { EditorHandle } from './request';
let focused: string | null = null;
const editors = new Map<string, () => EditorHandle | null>();
/** Insert into the last focused pane's editor without executing a statement. */
export function insertHistorySql(sql: string): boolean {
  const read = focused ? editors.get(focused) : [...editors.values()].at(-1);
  const editor = read?.();
  if (!editor) return false;
  const position = editor.cursor();
  const text = editor.text();
  editor.replace(text.slice(0, position) + sql + text.slice(position));
  editor.focus();
  return true;
}
/** Register a visible editor and remember the pane that last received focus. */
export function useHistoryInsert(
  id: string,
  editor: { current: EditorHandle | null },
  claim?: () => void,
): () => void {
  useEffect(() => {
    editors.set(id, () => editor.current);
    return () => {
      editors.delete(id);
      if (focused === id) focused = null;
    };
  }, [id, editor]);
  return () => {
    claim?.();
    focused = id;
  };
}
