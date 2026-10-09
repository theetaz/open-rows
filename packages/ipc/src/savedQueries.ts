// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  OpenedSqlFileDto,
  SavedQueryDto,
  SqlFileDto,
} from './generated/bindings';
import { unwrap } from './connections';

/** Saved queries of a connection, by name. */
export function listSavedQueries(profileId: string): Promise<SavedQueryDto[]> {
  return unwrap(commands.savedQueriesList(profileId));
}

/** Save SQL under a name on a connection. */
export function createSavedQuery(
  profileId: string,
  name: string,
  sql: string,
): Promise<SavedQueryDto> {
  return unwrap(commands.savedQueryCreate(profileId, name, sql));
}

/** Rename a saved query. */
export async function renameSavedQuery(
  id: string,
  name: string,
): Promise<void> {
  await unwrap(commands.savedQueryRename(id, name));
}

/** Delete a saved query. */
export async function deleteSavedQuery(id: string): Promise<void> {
  await unwrap(commands.savedQueryDelete(id));
}

/**
 * Save SQL As…: Rust shows the native save panel and writes the file.
 * Resolves the chosen file, or `null` when cancelled or outside the native shell.
 */
export async function saveSqlFileAs(
  title: string,
  sql: string,
): Promise<SqlFileDto | null> {
  if (!isTauri()) return null;
  return unwrap(commands.sqlFileSaveAs(title, sql));
}

/** Write a tab's SQL to the `.sql` file it is bound to. */
export async function saveSqlFile(path: string, sql: string): Promise<void> {
  await unwrap(commands.sqlFileSave(path, sql));
}

/**
 * Open SQL File…: Rust shows the native open panel and reads the file
 * (16 MiB cap, UTF-8). Resolves `null` when cancelled or outside the native shell.
 */
export async function openSqlFile(): Promise<OpenedSqlFileDto | null> {
  if (!isTauri()) return null;
  return unwrap(commands.sqlFileOpen());
}
