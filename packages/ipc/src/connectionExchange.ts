// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type {
  ImportDecisionDto,
  ImportPreview,
  ImportSource,
  ProfileDto,
} from './generated/bindings';
import { unwrap } from './connections';
/** Export selected profiles (empty means all) to a native-selected file. */
export function exportConnections(ids: string[]): Promise<string | null> {
  return unwrap(commands.connectionsExport(ids));
}
/** Read public files into a core-owned, unwritten conflict preview. */
export function previewConnections(
  source: ImportSource,
): Promise<ImportPreview | null> {
  return unwrap(commands.connectionsImportPreview(source));
}
/** Apply explicit per-row choices; the token is single-use and rejects stale previews. */
export function importConnections(
  token: string,
  decisions: ImportDecisionDto[],
): Promise<ProfileDto[]> {
  return unwrap(commands.connectionsImportApply(token, decisions));
}
/** Cancel a preview and immediately drop any imported passwords from memory. */
export async function cancelConnectionImport(token: string): Promise<void> {
  await unwrap(commands.connectionsImportCancel(token));
}
