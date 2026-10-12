// SPDX-License-Identifier: Apache-2.0
import type { ColumnChunk } from './tiles';

type Scalar = (chunk: ColumnChunk, type: string, row: number) => string;

/** Exact JSON display of Arrow document structs/lists, keeping integer digits. */
export function documentValue(
  chunk: ColumnChunk,
  type: string,
  row: number,
  scalar: Scalar,
): string {
  if (missing(chunk, row)) return 'null';
  if (chunk.fields)
    return `{${chunk.fields.map((field) => `${JSON.stringify(field.name)}:${documentValue(field.chunk, field.type, row, scalar)}`).join(',')}}`;
  if (chunk.child && chunk.offsets && type.startsWith('List<')) {
    const start = chunk.offsets[row] ?? 0;
    const end = chunk.offsets[row + 1] ?? start;
    const values: string[] = [];
    for (let index = start; index < end; index++)
      values.push(documentValue(chunk.child, type.slice(5, -1), index, scalar));
    return `[${values.join(',')}]`;
  }
  return scalar(chunk, type, row);
}
function missing(chunk: ColumnChunk, row: number): boolean {
  const at = chunk.offset + row;
  return (
    chunk.nullCount > 0 &&
    (!chunk.validity ||
      ((chunk.validity[at >>> 3] ?? 0) & (1 << (at & 7))) === 0)
  );
}
