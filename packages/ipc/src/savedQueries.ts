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

/**
 * Write a tab's SQL to the `.sql` file it is bound to; resolves the file's
 * new modification time (epoch ms), `null` when the platform has none.
 */
export function saveSqlFile(path: string, sql: string): Promise<number | null> {
  return unwrap(commands.sqlFileSave(path, sql));
}

/**
 * Modification time (epoch ms) of a tab's `.sql` file, to notice changes
 * made by another program; `null` when it is gone or outside the native shell.
 */
export async function sqlFileModified(path: string): Promise<number | null> {
  if (!isTauri()) return null;
  return unwrap(commands.sqlFileStat(path));
}

/** Read a tab's `.sql` file again (Reload after an outside change). */
export function readSqlFile(path: string): Promise<OpenedSqlFileDto> {
  return unwrap(commands.sqlFileRead(path));
}

/**
 * Open SQL File…: Rust shows the native open panel and reads the file
 * (16 MiB cap, UTF-8). Resolves `null` when cancelled or outside the native shell.
 */
export async function openSqlFile(): Promise<OpenedSqlFileDto | null> {
  if (!isTauri()) return null;
  return unwrap(commands.sqlFileOpen());
}
