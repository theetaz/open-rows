// SPDX-License-Identifier: Apache-2.0
import { Channel } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  ExecuteReply,
  ExecuteRequest,
  HistoryItemDto,
  EngineDto,
  LiteralRangeDto,
  HistorySearchRequest,
  QueryEventDto,
  SourceResolution,
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

/**
 * Entries of the local history, newest first. `profileId` null searches every
 * connection; the free text keeps statements whose words all start a word.
 */
export function searchHistory(
  request: HistorySearchRequest,
): Promise<HistoryItemDto[]> {
  return unwrap(commands.historySearch(request));
}

/** Delete one history entry; resolves `false` when it was already gone. */
export function deleteHistory(id: number): Promise<boolean> {
  return unwrap(commands.historyDelete(id));
}

/** Delete every history entry of one saved connection. */
export function clearHistory(profileId: string | null): Promise<null> {
  return unwrap(commands.historyClear(profileId));
}

/** SQL literal positions for review; no session is opened. */
export function historyLiterals(
  sql: string,
  engine: EngineDto,
): Promise<LiteralRangeDto[]> {
  return commands.historyLiterals(sql, engine);
}

/** Resolve an unqualified table on this query session without visible results or history. */
export function resolveQuerySource(
  sessionId: string,
  table: string,
): Promise<SourceResolution> {
  return unwrap(commands.queryResolveSource(sessionId, table));
}
