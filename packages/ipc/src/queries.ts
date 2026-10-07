// SPDX-License-Identifier: Apache-2.0
import { Channel } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  ExecuteReply,
  ExecuteRequest,
  HistoryItemDto,
  QueryEventDto,
} from './generated/bindings';
import { toCommandError, unwrap } from './connections';

/**
 * Split, safety-check and start SQL on a session. Events of a started run
 * arrive on `onEvent` in order and always end with `finished`; a
 * `confirmationRequired` or `blocked` reply means nothing ran.
 */
export function executeQuery(
  request: ExecuteRequest,
  onEvent: (event: QueryEventDto) => void,
): Promise<ExecuteReply> {
  const channel = new Channel<QueryEventDto>();
  channel.onmessage = onEvent;
  return unwrap(commands.queryExecute(request, channel));
}

/** Cancel a running query and the statements after it. */
export async function cancelQuery(queryId: string): Promise<void> {
  try {
    await commands.queryCancel(queryId);
  } catch (error) {
    throw toCommandError(error);
  }
}

/** Free results of a closed or re-run tab in the core's result store. */
export async function releaseResults(
  resultIds: readonly string[],
): Promise<void> {
  if (!resultIds.length) return;
  try {
    await commands.resultsRelease([...resultIds]);
  } catch (error) {
    throw toCommandError(error);
  }
}

/** The newest statements run on a saved connection, newest first. */
export function recentHistory(
  profileId: string,
  limit: number,
): Promise<HistoryItemDto[]> {
  return unwrap(commands.historyRecent(profileId, limit));
}
