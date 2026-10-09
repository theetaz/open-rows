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

/**
 * File-bound query tabs whose text differs from the file; a file changed by
 * another program waits for Reload or Keep instead of being overwritten.
 */
function unsavedFiles(state: TabsState): (QueryTab & { file: TabFile })[] {
  return state.tabs.filter(
    (tab): tab is QueryTab & { file: TabFile } =>
      tab.file !== undefined &&
      tab.file.external === undefined &&
      tab.sql !== tab.file.saved,
  );
}

/** When tabs are stored; nothing is written before the saved tabs loaded. */
type Persistence = { autoSave: boolean; restore: boolean; loaded: boolean };

/**
 * Store open tabs for session restore in the local store (query text only
 * with Auto Save) and, with Auto Save, write file-bound tabs back to their
 * `.sql` files, debounced. `onFile` records each write's outcome on the tab.
 */
export function useTabPersistence(
  state: TabsState,
  { autoSave, restore, loaded }: Persistence,
  onFile: (id: string, file: TabFile) => void,
): void {
  const latest = useRef({ state, autoSave, restore, loaded });
  useEffect(() => {
    latest.current = { state, autoSave, restore, loaded };
    if (!loaded) return;
    const timer = window.setTimeout(() => {
      writeTabs(state, { autoSave, restore });
    }, AUTO_SAVE_DELAY_MS);
    return () => {
      window.clearTimeout(timer);
    };
  }, [state, autoSave, restore, loaded]);
  useEffect(() => {
    const flush = (): void => {
      const now = latest.current;
      if (now.loaded)
        writeTabs(now.state, { autoSave: now.autoSave, restore: now.restore });
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
            (modifiedMs) => {
              onFile(tab.id, {
                ...file,
                saved: tab.sql,
                ...(modifiedMs === null ? {} : { modifiedMs }),
              });
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
