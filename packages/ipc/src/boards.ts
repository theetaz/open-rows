// SPDX-License-Identifier: Apache-2.0
// Analytics boards (M25-06): validated definitions and bounded read-only widget runs.
import { commands } from './generated/bindings';
import type {
  Board,
  BoardDto,
  BoardRun,
  BoardRunReply,
} from './generated/bindings';
import { unwrap } from './connections';

export type { BoardSave } from './generated/bindings';

/** Boards of a connection, by name; imported boards report `reviewed: false`. */
export function listBoards(profileId: string): Promise<BoardDto[]> {
  return unwrap(commands.boardsList(profileId));
}
/** Create (no id) or replace a board after native validation. */
export function saveBoard(
  profileId: string,
  id: string | null,
  board: Board,
): Promise<BoardDto> {
  return unwrap(commands.boardSave({ profileId, id, board }));
}
/** Import portable JSON; the board stays paused until approved. */
export function importBoard(
  profileId: string,
  text: string,
): Promise<BoardDto> {
  return unwrap(commands.boardImport(profileId, text));
}
/** Portable JSON of a board's definitions, without results or secrets. */
export function exportBoard(profileId: string, id: string): Promise<string> {
  return unwrap(commands.boardExport(profileId, id));
}
/** Record that the user reviewed an imported board's queries. */
export async function approveBoard(
  profileId: string,
  id: string,
): Promise<void> {
  await unwrap(commands.boardApprove(profileId, id));
}
/** Delete a board. */
export async function deleteBoard(
  profileId: string,
  id: string,
): Promise<void> {
  await unwrap(commands.boardDelete(profileId, id));
}
/** Run one widget read-only with bounded rows, bytes and time. */
export function runBoardWidget(request: BoardRun): Promise<BoardRunReply> {
  return unwrap(commands.boardRun(request));
}

export type {
  Board,
  BoardColumn,
  BoardDto,
  BoardParam,
  BoardResult,
  BoardRun,
  BoardRunReply,
  ChartMapping,
  ColumnKind,
  Mark,
  ParamKind,
  ParamValue,
  ReadOnlyMode,
  Truncation,
  Widget,
  WidgetKind,
  WidgetLimits,
} from './generated/bindings';
