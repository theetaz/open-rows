// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { saveSqlFile } from '@openrows/ipc';
import { writeTabs } from './tabStorage';
import type { QueryTab, TabFile, TabsState } from './tabs';

/**
 * Editor text reaches the tab state within 300 ms; storing it this much later
 * keeps unsaved text persisted within about a second of typing.
 */
const AUTO_SAVE_DELAY_MS = 500;

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** File-bound query tabs whose text differs from the file. */
function unsavedFiles(state: TabsState): (QueryTab & { file: TabFile })[] {
  return state.tabs.filter(
    (tab): tab is QueryTab & { file: TabFile } =>
      tab.file !== undefined && tab.sql !== tab.file.saved,
  );
}

/**
 * Store open tabs for session restore (query text only with Auto Save) and,
 * with Auto Save, write file-bound tabs back to their `.sql` files, debounced. `onFile`
 * records each write's outcome on the tab.
 */
export function useTabPersistence(
  state: TabsState,
  autoSave: boolean,
  restore: boolean,
  onFile: (id: string, file: TabFile) => void,
): void {
  const latest = useRef({ state, autoSave, restore });
  useEffect(() => {
    latest.current = { state, autoSave, restore };
    const timer = window.setTimeout(() => {
      writeTabs(state, { autoSave, restore });
    }, AUTO_SAVE_DELAY_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [state, autoSave, restore]);
  useEffect(() => {
    const flush = (): void => {
      const { autoSave: text, restore: keep } = latest.current;
      writeTabs(latest.current.state, { autoSave: text, restore: keep });
    };
    window.addEventListener('pagehide', flush);
    return () => {
      window.removeEventListener('pagehide', flush);
      flush();
    };
  }, []);
  const writing = useRef(new Set<string>());
  useEffect(() => {
    if (!autoSave) return;
    const timer = window.setTimeout(() => {
      for (const tab of unsavedFiles(state)) {
        if (tab.file.error || writing.current.has(tab.id)) continue;
        writing.current.add(tab.id);
        const file = { path: tab.file.path, name: tab.file.name };
        saveSqlFile(file.path, tab.sql)
          .then(
            () => {
              onFile(tab.id, { ...file, saved: tab.sql });
            },
            (error: unknown) => {
              onFile(tab.id, { ...tab.file, error: message(error) });
            },
          )
          .finally(() => writing.current.delete(tab.id));
      }
    }, AUTO_SAVE_DELAY_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [state, autoSave, onFile]);
}
