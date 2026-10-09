// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type { ProjectDto } from './generated/bindings';
import { unwrap } from './connections';

/**
 * Open Project…: Rust shows the native folder panel and opens the folder as
 * a project, creating `.openrows/project.yaml` when it has none. Resolves
 * `null` when cancelled or outside the native shell.
 */
export async function chooseProject(): Promise<ProjectDto | null> {
  if (!isTauri()) return null;
  return unwrap(commands.projectChoose());
}

/** Read a project's manifest and query tree (a recent project, or after a change). */
export function loadProject(root: string): Promise<ProjectDto> {
  return unwrap(commands.projectLoad(root));
}

/** Set or clear the connection the project's queries open on. */
export function setProjectConnection(
  root: string,
  profileId: string | null,
): Promise<ProjectDto> {
  return unwrap(commands.projectSetDefaultConnection(root, profileId));
}

/** New folder `name` in `parent` (relative paths); resolves its path. */
export function createProjectFolder(
  root: string,
  parent: string,
  name: string,
): Promise<string> {
  return unwrap(commands.projectCreateFolder(root, parent, name));
}

/** New `.sql` query in `folder` holding `sql`; resolves its path. */
export function createProjectQuery(
  root: string,
  folder: string,
  name: string,
  sql: string,
): Promise<string> {
  return unwrap(commands.projectCreateQuery(root, folder, name, sql));
}

/** Rename a project folder or query; resolves its new path. */
export function renameProjectItem(
  root: string,
  path: string,
  name: string,
): Promise<string> {
  return unwrap(commands.projectRename(root, path, name));
}

/** Move a project folder or query into `folder`; resolves its new path. */
export function moveProjectItem(
  root: string,
  path: string,
  folder: string,
): Promise<string> {
  return unwrap(commands.projectMove(root, path, folder));
}

/** Copy a project folder or query beside itself; resolves the copy's path. */
export function duplicateProjectItem(
  root: string,
  path: string,
): Promise<string> {
  return unwrap(commands.projectDuplicate(root, path));
}

/** Delete a project query, or a folder with everything in it. */
export async function deleteProjectItem(
  root: string,
  path: string,
): Promise<void> {
  await unwrap(commands.projectDelete(root, path));
}

/** Show a project folder or query in the file manager. */
export async function revealProjectItem(
  root: string,
  path: string,
): Promise<void> {
  await unwrap(commands.projectReveal(root, path));
}
