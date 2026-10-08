// SPDX-License-Identifier: Apache-2.0
const numeric =
  /^(int[248]?|smallint|integer|bigint|float[48]|real|double precision|numeric|decimal|money|oid|serial[248]?|bigserial|smallserial)$/i;

/** Right-align numeric database types in grids, like TablePlus. */
export function columnAlign(type: string | null): 'start' | 'end' {
  return type && numeric.test(type.replace(/\(.*\)$/, '')) ? 'end' : 'start';
}
