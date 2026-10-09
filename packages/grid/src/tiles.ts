import type { DisplayTile } from './displayTile';
// SPDX-License-Identifier: Apache-2.0
/** Clone-safe Arrow buffers; offset applies to validity and packed booleans. */
export interface ColumnChunk {
  length: number;
  offset: number;
  stride: number;
  nullCount: number;
  values: ArrayBufferView;
  validity: Uint8Array | undefined;
  offsets: Int32Array | undefined;
  /** Element values of a `List` chunk; `offsets` index into them. */
  child?: ColumnChunk | undefined;
}
/** A typed column with exact type description and extension metadata. */
export interface TypedColumn {
  name: string;
  type: string;
  metadata: Record<string, string>;
  chunks: ColumnChunk[];
}
/** A worker-decoded tile; no per-cell conversion or Arrow prototypes cross back. */
export interface Tile {
  rows: number;
  columns: TypedColumn[];
}
/** Decode completion includes the worker's own elapsed decode time. */
export interface DecodedTile {
  tile: Tile;
  decodeMs: number;
}
/** Worker protocol distinguishes readiness, decode success and typed failures. */
export type WorkerReply =
  | { kind: 'ready' }
  | { kind: 'decoded'; id: number; result: DecodedTile }
  | { kind: 'display'; id: number; result: DisplayTile }
  | { kind: 'error'; id: number; message: string };
