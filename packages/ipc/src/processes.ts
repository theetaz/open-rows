// SPDX-License-Identifier: Apache-2.0
import { commands } from './generated/bindings';
import type { ProcessDto, ProcessSignalRequest } from './generated/bindings';
import { unwrap } from './connections';

/**
 * Client sessions on the server of a connected profile, the longest running
 * first, read on the database's metadata connection.
 */
export function listProcesses(
  profileId: string,
  database: string,
): Promise<ProcessDto[]> {
  return unwrap(commands.processesList(profileId, database));
}

/**
 * Cancel a process's statement or terminate it. Callers confirm first; the
 * core refuses read-only connections and, without `allowOwn`, the connection
 * the list runs on.
 */
export async function signalProcess(
  request: ProcessSignalRequest,
): Promise<void> {
  await unwrap(commands.processSignal(request));
}
