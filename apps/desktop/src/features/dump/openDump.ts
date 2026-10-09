// SPDX-License-Identifier: Apache-2.0
import { pickRestoreFile } from '@openrows/ipc';
import { showDump } from './dumpStore';
import type { DumpTarget } from './dumpStore';

/** File ▸ Back Up Database…: the backup sheet of the target's database. */
export function openBackup(target: DumpTarget): void {
  showDump({ kind: 'backup', target });
}

/**
 * File ▸ Restore Database…: the open panel (a file, or a `pg_dump`
 * directory with `folder`), then the restore sheet for the picked backup.
 */
export async function pickRestoreFor(
  target: DumpTarget,
  folder = false,
): Promise<void> {
  const file = await pickRestoreFile(folder);
  if (file) showDump({ kind: 'restore', target, file });
}
