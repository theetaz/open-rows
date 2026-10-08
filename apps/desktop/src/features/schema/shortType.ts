// SPDX-License-Identifier: Apache-2.0
/** PostgreSQL's internal short names, as TablePlus lists column types. */
const aliases: readonly (readonly [RegExp, string])[] = [
  [/^integer$/, 'int4'],
  [/^bigint$/, 'int8'],
  [/^smallint$/, 'int2'],
  [/^double precision$/, 'float8'],
  [/^real$/, 'float4'],
  [/^boolean$/, 'bool'],
  [/^character varying\b/, 'varchar'],
  [/^character\b/, 'bpchar'],
  [/^timestamp(\s?\(\d+\))? without time zone$/, 'timestamp'],
  [/^timestamp(\s?\(\d+\))? with time zone$/, 'timestamptz'],
  [/^time(\s?\(\d+\))? without time zone$/, 'time'],
  [/^time(\s?\(\d+\))? with time zone$/, 'timetz'],
  [/^numeric\b/, 'numeric'],
  [/^bit varying\b/, 'varbit'],
];

/** Compact type label (`double precision` → `float8`); arrays keep `[]`. */
export function shortType(type: string): string {
  const array = type.endsWith('[]');
  const base = array ? type.slice(0, -2) : type;
  const short = aliases.find(([pattern]) => pattern.test(base))?.[1] ?? base;
  return array ? `${short}[]` : short;
}
