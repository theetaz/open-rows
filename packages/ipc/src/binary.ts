// SPDX-License-Identifier: Apache-2.0
import { invoke } from '@tauri-apps/api/core';
import type { WindowRequest } from './generated/bindings';

/** Normalize Tauri raw bytes, including the macOS postMessage number-array fallback. */
export async function resultWindow(
  request: WindowRequest,
): Promise<ArrayBuffer> {
  const bytes = await invoke<unknown>('result_window', { request });
  if (bytes instanceof ArrayBuffer) return bytes;
  if (Array.isArray(bytes) && bytes.every(isByte))
    return Uint8Array.from(bytes).buffer;
  throw new Error('Expected Arrow IPC bytes (ArrayBuffer or byte array)');
}

function isByte(value: unknown): value is number {
  return (
    typeof value === 'number' &&
    Number.isInteger(value) &&
    value >= 0 &&
    value <= 255
  );
}
