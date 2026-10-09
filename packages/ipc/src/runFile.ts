// SPDX-License-Identifier: Apache-2.0
import { Channel, isTauri } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type {
  PickedScriptDto,
  RunFileCheckDto,
  RunFileCheckRequest,
  RunFileEventDto,
  RunFileStartRequest,
  RunFileStarted,
} from './generated/bindings';
import { unwrap } from './connections';

/**
 * Choose a `.sql` file in the native open panel. Resolves `null` when the
 * panel was cancelled or outside the native shell.
 */
export async function pickRunFile(): Promise<PickedScriptDto | null> {
  if (!isTauri()) return null;
  return unwrap(commands.runFilePick());
}

/**
 * Read a picked file once and judge every statement under the session's
 * connection safety switches; nothing runs.
 */
export function checkRunFile(
  request: RunFileCheckRequest,
): Promise<RunFileCheckDto> {
  return unwrap(commands.runFileCheck(request));
}

/**
 * Run a checked file on a session of its own. Events arrive on `onEvent`
 * and end with `finished`; cancel with `cancelQuery(reply.runId)`.
 */
export function startRunFile(
  request: RunFileStartRequest,
  onEvent: (event: RunFileEventDto) => void,
): Promise<RunFileStarted> {
  const channel = new Channel<RunFileEventDto>();
  channel.onmessage = onEvent;
  return unwrap(commands.runFileStart(request, channel));
}
