// SPDX-License-Identifier: Apache-2.0
import { useState } from 'react';
import { openSqlFile, saveSqlFile, saveSqlFileAs } from '@openrows/ipc';
import type { EditorHandle, FileAction } from './request';
import type { QueryTab, TabFile } from './tabs';
import type { QueryTabs } from './useQueryTabs';

/** What a query tab needs from the tab set for its files. */
export type TabFileActions = Pick<QueryTabs, 'open' | 'bindFile' | 'edit'>;

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

/** A file binding with its text and, when known, its modification time. */
export function tabFile(
  file: { path: string; name: string },
  saved: string,
  modifiedMs: number | null | undefined,
): TabFile {
  const { path, name } = file;
  return modifiedMs == null
    ? { path, name, saved }
    : { path, name, saved, modifiedMs };
}

/**
 * Save (⌘S), Save SQL As… (⇧⌘S) and Open SQL File… (⌘O) for one query tab.
 * Saving binds the tab to the file; `failure` is the last error, including
 * an Auto Save write that failed.
 */
export function useTabFiles(
  tab: QueryTab,
  editor: { current: EditorHandle | null },
  tabs: TabFileActions,
): { act: (action: FileAction) => void; failure: string | null } {
  const [failure, setFailure] = useState<string | null>(null);
  const fail = (error: unknown): void => {
    setFailure(`Could not save: ${message(error)}`);
  };
  const bind = (
    file: { path: string; name: string },
    saved: string,
    modifiedMs: number | null | undefined,
  ): void => {
    setFailure(null);
    tabs.bindFile(tab.id, tabFile(file, saved, modifiedMs));
  };
  const saveAs = (sql: string): void => {
    saveSqlFileAs(tab.title, sql).then((file) => {
      if (file) bind(file, sql, file.modifiedMs);
    }, fail);
  };
  const save = (sql: string): void => {
    const { file } = tab;
    if (!file) {
      saveAs(sql);
      return;
    }
    saveSqlFile(file.path, sql).then((modifiedMs) => {
      bind(file, sql, modifiedMs);
    }, fail);
  };
  const open = (): void => {
    openSqlFile().then(
      (opened) => {
        if (!opened) return;
        const { file, sql } = opened;
        const scope = {
          connectionId: tab.connectionId,
          database: tab.database,
        };
        tabs.open(scope, {
          title: file.name,
          sql,
          file: tabFile(file, sql, file.modifiedMs),
        });
      },
      (error: unknown) => {
        setFailure(`Could not open: ${message(error)}`);
      },
    );
  };
  const act = (action: FileAction): void => {
    const sql = editor.current?.text() ?? tab.sql;
    // The saved text must match the tab's, or Auto Save would write it again.
    if (action !== 'open') tabs.edit(tab.id, sql);
    if (action === 'save') save(sql);
    else if (action === 'saveAs') saveAs(sql);
    else open();
  };
  const autoSaveError = tab.file?.error
    ? `Could not save ${tab.file.name}: ${tab.file.error}`
    : null;
  return { act, failure: failure ?? autoSaveError };
}
