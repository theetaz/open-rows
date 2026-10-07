// SPDX-License-Identifier: Apache-2.0
import { Channel } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  EditInfoDto,
  EditReply,
  CountReply,
  CountRequest,
  PageReply,
  PageRequest,
  QueryEventDto,
  SqlPreviewDto,
  TableQueryDto,
  TableStructureDto,
  TableTarget,
} from './generated/bindings';
import { unwrap } from './connections';

/** Columns, indexes and planner row estimate of a relation (metadata session). */
export function tableStructure(
  target: TableTarget,
): Promise<TableStructureDto> {
  return unwrap(commands.tableStructure(target));
}

/**
 * Start one page of a table view on the connection's session. Events arrive
 * on `onEvent` like a query run and end with `finished`; cancel with
 * `cancelQuery(reply.queryId)`.
 */
export function tablePage(
  request: PageRequest,
  onEvent: (event: QueryEventDto) => void,
): Promise<PageReply> {
  const channel = new Channel<QueryEventDto>();
  channel.onmessage = onEvent;
  return unwrap(commands.tablePage(request, channel));
}

/** Exact row count with the view's filters; `cancelQuery(request.ticket)` stops it. */
export function tableCount(request: CountRequest): Promise<CountReply> {
  return unwrap(commands.tableCount(request));
}

/** The page SELECT and its bind parameters, without running it. */
export function tableSql(
  query: TableQueryDto,
  offset: number,
  limit: number,
): Promise<SqlPreviewDto> {
  return unwrap(commands.tableSql(query, offset, limit));
}

/** Catalog identity and write restrictions for a table. */
export function tableEditInfo(target: TableTarget): Promise<EditInfoDto> {
  return unwrap(commands.tableEditInfo(target));
}
/** Exact original values for one stable key, with no formatting loss. */
export function tableEditRow(
  target: TableTarget,
  keys: Record<string, string>,
): Promise<Record<string, string | null>> {
  return unwrap(commands.tableEditRow(target, keys));
}
/** Review bound SQL, then commit only the reviewed payload atomically. */
export function tableEdit(
  request: import('./generated/bindings').EditRequest,
): Promise<EditReply> {
  return unwrap(commands.tableEdit(request));
}
