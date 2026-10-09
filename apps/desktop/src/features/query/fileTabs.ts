// SPDX-License-Identifier: Apache-2.0
import { readSqlFile } from '@openrows/ipc';
import type { TabScope } from './tabs';
import { tabFile } from './useTabFiles';
import type { QueryTabs } from './useQueryTabs';

/**
 * Open a `.sql` file (e.g. a project query) in a tab of `scope` bound to it,
 * or focus the tab that already shows it.
 */
export function openQueryFile(
  tabs: QueryTabs,
  scope: TabScope,
  path: string,
): Promise<void> {
  const open = tabs.tabsFor(scope).find((tab) => tab.file?.path === path);
  if (open) {
    tabs.select(open.id);
    return Promise.resolve();
  }
  return readSqlFile(path).then(({ file, sql }) => {
    tabs.open(scope, {
      title: file.name,
      sql,
      file: tabFile(file, sql, file.modifiedMs),
    });
  });
}

/** `path` after `from` moved to `to`, or `null` when it is not under `from`. */
export function movedPath(
  path: string,
  from: string,
  to: string,
): string | null {
  if (path === from) return to;
  const separator = from.includes('\\') && !from.includes('/') ? '\\' : '/';
  return path.startsWith(`${from}${separator}`)
    ? `${to}${path.slice(from.length)}`
    : null;
}

/** Tabs bound to a file at or under `from` follow it to `to` (rename, move). */
export function rebindFiles(tabs: QueryTabs, from: string, to: string): void {
  for (const tab of tabs.state.tabs) {
    const { file } = tab;
    const path = file ? movedPath(file.path, from, to) : null;
    if (!file || path === null) continue;
    const name = path.slice(
      Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\')) + 1,
    );
    tabs.bindFile(tab.id, { ...file, path, name });
  }
}
