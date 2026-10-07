// SPDX-License-Identifier: Apache-2.0
import { tableFromIPC, DataType } from 'apache-arrow';
import type { DecodedTile } from './types';

/** Decode in a worker only; expose typed buffers without lossy JS cell coercion. */
export function decodeTile(bytes: ArrayBuffer): DecodedTile {
  const start = performance.now();
  validateStream(bytes);
  const table = tableFromIPC(new Uint8Array(bytes));
  const columns = table.schema.fields.map((field, index) => {
    const vector = table.getChildAt(index);
    if (!vector) throw new Error(`Missing Arrow column ${String(index)}`);
    return {
      name: field.name,
      type: describeType(field.type),
      metadata: Object.fromEntries(field.metadata),
      chunks: vector.data.map((chunk) => ({
        length: chunk.length,
        offset: chunk.offset,
        stride: chunk.stride,
        nullCount: chunk.nullCount,
        values: chunk.values as ArrayBufferView,
        validity: chunk.nullBitmap,
        offsets: chunk.valueOffsets as Int32Array | undefined,
      })),
    };
  });
  return {
    tile: { rows: table.numRows, columns },
    decodeMs: performance.now() - start,
  };
}

function describeType(type: unknown): string {
  if (
    DataType.isInt(type) ||
    DataType.isFloat(type) ||
    DataType.isBool(type) ||
    DataType.isUtf8(type) ||
    DataType.isTimestamp(type) ||
    DataType.isDecimal(type) ||
    DataType.isBinary(type) ||
    DataType.isDate(type) ||
    DataType.isFixedSizeBinary(type)
  )
    return type.toString();
  throw new Error('Unsupported Arrow tile type');
}

function validateStream(bytes: ArrayBuffer): void {
  if (bytes.byteLength < 16) throw new Error('Truncated Arrow IPC stream');
  const view = new DataView(bytes);
  const metadataLength = view.getUint32(4, true);
  if (
    view.getUint32(0, true) !== 0xffffffff ||
    metadataLength === 0 ||
    metadataLength > bytes.byteLength - 8 ||
    view.getUint32(bytes.byteLength - 8, true) !== 0xffffffff ||
    view.getUint32(bytes.byteLength - 4, true) !== 0
  )
    throw new Error('Invalid Arrow IPC framing');
}
