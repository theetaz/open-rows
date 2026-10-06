// SPDX-License-Identifier: Apache-2.0
import type { DecodedTile } from './types';

/** Transfer each shared IPC buffer once; fixed-width columns have no offsets. */
export function transferBuffers(result: DecodedTile): ArrayBuffer[] {
  const unique = new Set<ArrayBuffer>();
  for (const column of result.tile.columns) {
    for (const chunk of column.chunks) {
      for (const view of [chunk.values, chunk.validity, chunk.offsets]) {
        if (
          view &&
          Object.prototype.toString.call(view.buffer) === '[object ArrayBuffer]'
        )
          unique.add(view.buffer as ArrayBuffer);
      }
    }
  }
  return [...unique];
}
