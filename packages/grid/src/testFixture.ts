// SPDX-License-Identifier: Apache-2.0
import type { WindowRequest } from '@openrows/ipc';
import type { Tile } from './tiles';
import { TileSource } from './source';
import { gridTheme } from './theme';
import type { GridProps } from './model';
/** Deterministic typed buffers, no node-canvas or million JS row objects. */
export function tile(request: WindowRequest): Promise<Tile> {
  return Promise.resolve({
    rows: request.rowCount,
    columns: Array.from({ length: request.colCount }, (_, column) => ({
      name: String(column),
      type: 'Int32',
      metadata: {},
      chunks: [
        {
          length: request.rowCount,
          offset: 0,
          stride: 1,
          nullCount: 1,
          values: Int32Array.from(
            { length: request.rowCount },
            (_, row) => request.rowStart + row,
          ),
          validity: Uint8Array.from(
            { length: Math.ceil(request.rowCount / 8) },
            (_, index) => (index === 0 ? 254 : 255),
          ),
          offsets: undefined,
        },
      ],
    })),
  });
}
/** Both renderers receive the same shape and token theme. */
export function props(mode: 'light' | 'dark' = 'light'): GridProps {
  return {
    source: new TileSource(
      'spike',
      1_000_000,
      Array.from({ length: 100 }, (_, index) => ({
        id: String(index),
        title: `column_${String(index)}`,
        width: 120,
        pinned: index < 2,
      })),
      tile,
    ),
    selection: {
      row: 2,
      column: 2,
      ranges: [
        { r0: 1, r1: 3, c0: 1, c1: 3 },
        { r0: 5, r1: 6, c0: 5, c1: 6 },
      ],
    },
    theme: gridTheme(mode),
    onSelection: () => {},
    onError: () => {},
  };
}
