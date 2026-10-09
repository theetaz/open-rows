// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type { WorkspacePartDto } from './generated/bindings';
import { unwrap } from './connections';

/**
 * The saved session or workspace state of the main window (open query tabs,
 * open connections), from the local store; `null` when nothing is saved or
 * outside the native shell.
 */
export async function readWorkspaceState(
  part: WorkspacePartDto,
): Promise<string | null> {
  if (!isTauri()) return null;
  return unwrap(commands.workspaceStateRead(part));
}

/** Save one part of the window's state; `null` forgets it. */
export async function writeWorkspaceState(
  part: WorkspacePartDto,
  state: string | null,
): Promise<void> {
  if (!isTauri()) return;
  await unwrap(commands.workspaceStateWrite(part, state));
}
