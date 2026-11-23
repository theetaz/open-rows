// SPDX-License-Identifier: Apache-2.0
const numeric =
  /^(u?int(2|4|8|16|32|64|128|256)?|u?(tiny|small|medium|big|huge)int|integer|uinteger|float(4|8|32|64)?|real|double( precision)?|numeric|number|decimal(32|64|128|256)?|dec|fixed|(small)?money|oid|(small|big)?serial[248]?)$/;

/** The bare type name: no `Nullable(…)` wrapper, parameters or MySQL modifiers. */
function baseType(type: string): string {
  return type
    .trim()
    .toLowerCase()
    .replace(/^nullable\((.*)\)$/, '$1')
    .replace(/\(.*\)/, '')
    .replace(/\s+(unsigned|signed|zerofill)\b/g, '')
    .trim();
}

/** Right-align numeric database types in grids, like TablePlus and the kit's grid. */
export function columnAlign(type: string | null): 'start' | 'end' {
  return type && numeric.test(baseType(type)) ? 'end' : 'start';
}
