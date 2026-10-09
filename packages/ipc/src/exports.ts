// SPDX-License-Identifier: Apache-2.0
import { Channel } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  ExportEventDto,
  ExportReply,
  ExportRequest,
} from './generated/bindings';
import { unwrap } from './connections';

/**
 * Validate an export, let the user choose the file in the native save panel
 * and stream the rows to it in the core. A `started` reply is followed by
 * progress on `onEvent`, which ends with `finished`, `failed` or
 * `cancelled`; cancel with `cancelQuery(reply.exportId)`. `dismissed` means
 * the panel was cancelled and nothing was written.
 */
export function startExport(
  request: ExportRequest,
  onEvent: (event: ExportEventDto) => void,
): Promise<ExportReply> {
  const channel = new Channel<ExportEventDto>();
  channel.onmessage = onEvent;
  return unwrap(commands.exportStart(request, channel));
}

/** Show a finished export's file or folder selected in Finder. */
export async function revealExport(path: string): Promise<void> {
  await unwrap(commands.exportReveal(path));
}

/** Open a finished export's file (or folder) with its default application. */
export async function openExport(path: string): Promise<void> {
  await unwrap(commands.exportOpen(path));
}
