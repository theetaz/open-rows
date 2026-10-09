// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef } from 'react';
import { readSqlFile, sqlFileModified } from '@openrows/ipc';
import { tabFile } from './useTabFiles';
import type { QueryTab, TabFile } from './tabs';

type FileTab = QueryTab & { file: TabFile };

/**
 * Compare a tab's file with what the tab last read or wrote. A new
 * modification time with the same text (or the tab's own text) is recorded
 * quietly; other text is offered as `external` for Reload or Keep.
 */
async function checkFile(
  tab: FileTab,
  onFile: (id: string, file: TabFile) => void,
): Promise<void> {
  const { file } = tab;
  const modified = await sqlFileModified(file.path);
  if (modified === null || modified === file.modifiedMs) return;
  const { sql } = await readSqlFile(file.path);
  const same = sql === file.saved || sql === tab.sql;
  const stamped = tabFile(file, same ? sql : file.saved, modified);
  onFile(tab.id, same ? stamped : { ...stamped, external: sql });
}

/**
 * Notice `.sql` files changed by another program (M1-21): every file-bound
 * tab is checked once its saved tabs loaded and whenever the window gains
 * focus, like TablePlus and VS Code.
 */
export function useFileChanges(
  tabs: readonly QueryTab[],
  loaded: boolean,
  onFile: (id: string, file: TabFile) => void,
): void {
  const latest = useRef(tabs);
  useEffect(() => {
    latest.current = tabs;
  });
  useEffect(() => {
    if (!loaded) return;
    const check = (): void => {
      for (const tab of latest.current)
        if (tab.file)
          checkFile({ ...tab, file: tab.file }, onFile).catch(() => {
            /* A file that cannot be read is reported when it is saved. */
          });
    };
    check();
    window.addEventListener('focus', check);
    return () => {
      window.removeEventListener('focus', check);
    };
  }, [loaded, onFile]);
}
