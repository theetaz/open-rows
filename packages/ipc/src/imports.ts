// SPDX-License-Identifier: Apache-2.0
import { Channel } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  ImportEventDto,
  ImportFileReply,
  ImportPreviewDto,
  ImportPreviewRequest,
  ImportReply,
  ImportRequest,
} from './generated/bindings';
import { unwrap } from './connections';

/**
 * Show the native open panel for a CSV, TSV, JSON or NDJSON file. The path
 * stays in the core; `picked` carries the id later calls name it by.
 */
export function pickImportFile(): Promise<ImportFileReply> {
  return unwrap(commands.importPickFile());
}

/**
 * Detect (`options: null`) or apply parse options and read the file's
 * columns with inferred types, its first rows and the cells the given
 * target types would reject.
 */
export function previewImport(
  request: ImportPreviewRequest,
): Promise<ImportPreviewDto> {
  return unwrap(commands.importPreview(request));
}

/**
 * Start a dry run or an import on a dedicated connection. Progress arrives
 * on `onEvent` and ends with `finished`, `failed` or `cancelled`; stop it
 * with `cancelImport(reply.importId)`.
 */
export function startImport(
  request: ImportRequest,
  onEvent: (event: ImportEventDto) => void,
): Promise<ImportReply> {
  const channel = new Channel<ImportEventDto>();
  channel.onmessage = onEvent;
  return unwrap(commands.importStart(request, channel));
}

/** Cancel a running dry run or import; uncommitted rows are rolled back. */
export function cancelImport(importId: string): Promise<void> {
  return commands.importCancel(importId);
}
