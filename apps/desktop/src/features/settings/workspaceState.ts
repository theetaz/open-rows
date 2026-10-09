// SPDX-License-Identifier: Apache-2.0
import { readWorkspaceState, writeWorkspaceState } from '@openrows/ipc';
import type { WorkspacePartDto } from '@openrows/ipc';

/** Web-storage keys each part lived in before the local store (M1-26). */
const LEGACY_KEYS: Readonly<Record<WorkspacePartDto, string>> = {
  queryTabs: 'openrows.queryTabs',
  session: 'openrows.session',
};

function legacyText(part: WorkspacePartDto): string | null {
  try {
    return localStorage.getItem(LEGACY_KEYS[part]);
  } catch {
    return null;
  }
}

function dropLegacy(part: WorkspacePartDto): void {
  try {
    localStorage.removeItem(LEGACY_KEYS[part]);
  } catch {
    /* Nothing to clear without web storage. */
  }
}

/**
 * A part of the window's saved state from the local store. The first time
 * after the move from web storage, its old value is imported into the store
 * and then cleared; until the import succeeds web storage keeps it.
 */
export async function loadWorkspaceState(
  part: WorkspacePartDto,
): Promise<string | null> {
  const legacy = legacyText(part);
  let stored: string | null;
  try {
    stored = await readWorkspaceState(part);
    if (legacy !== null && stored === null)
      await writeWorkspaceState(part, legacy);
  } catch {
    return legacy;
  }
  if (legacy !== null) dropLegacy(part);
  return stored ?? legacy;
}

/** Save a part in the background (`null` forgets it); typing never waits. */
export function saveWorkspaceState(
  part: WorkspacePartDto,
  state: string | null,
): void {
  writeWorkspaceState(part, state).catch(() => {
    /* The store is unavailable: the state lasts for this session only. */
  });
}
