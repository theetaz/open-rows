// SPDX-License-Identifier: Apache-2.0
import { invoke } from '@tauri-apps/api/core';
import { toCommandError } from './connections';
import type { WindowRequest } from './generated/bindings';

/** Arrow IPC bytes of one result window (`result_window`, raw response). */
export async function resultWindow(
  request: WindowRequest,
): Promise<ArrayBuffer> {
  return rawBytes(
    await invoke<unknown>('result_window', { request }).catch(
      (error: unknown) => {
        throw toCommandError(error);
      },
    ),
  );
}

/**
 * Normalize a raw Tauri reply to an ArrayBuffer, including the macOS
 * postMessage number-array fallback.
 */
export function rawBytes(reply: unknown): ArrayBuffer {
  if (reply instanceof ArrayBuffer) return reply;
  if (Array.isArray(reply) && reply.every(isByte))
    return Uint8Array.from(reply).buffer;
  throw new Error('Expected raw bytes (ArrayBuffer or byte array)');
}

function isByte(value: unknown): value is number {
  return (
    typeof value === 'number' &&
    Number.isInteger(value) &&
    value >= 0 &&
    value <= 255
  );
}
