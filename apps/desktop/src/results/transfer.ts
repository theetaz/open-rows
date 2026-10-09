// SPDX-License-Identifier: Apache-2.0
import type { ColumnChunk, DecodedTile } from './types';

/** Transfer each shared IPC buffer once; fixed-width columns have no offsets. */
export function transferBuffers(result: DecodedTile): ArrayBuffer[] {
  const unique = new Set<ArrayBuffer>();
  for (const column of result.tile.columns) {
    for (const chunk of column.chunks) collect(chunk, unique);
  }
  return [...unique];
}

/** A chunk's buffers and those of its list elements. */
function collect(chunk: ColumnChunk, unique: Set<ArrayBuffer>): void {
  for (const view of [chunk.values, chunk.validity, chunk.offsets]) {
    if (
      view &&
      Object.prototype.toString.call(view.buffer) === '[object ArrayBuffer]'
    )
      unique.add(view.buffer as ArrayBuffer);
  }
  if (chunk.child) collect(chunk.child, unique);
}
