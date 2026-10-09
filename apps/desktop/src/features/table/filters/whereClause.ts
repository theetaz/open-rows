// SPDX-License-Identifier: Apache-2.0

/**
 * The `WHERE …` part of a generated, unsorted page `SELECT`; `null` when
 * the statement has no filters. The generator writes the first ` WHERE `
 * (column names are quoted) and the final ` LIMIT ` itself.
 */
export function whereClause(sql: string): string | null {
  const start = sql.indexOf(' WHERE ');
  if (start < 0) return null;
  const rest = sql.slice(start + 1);
  const end = rest.lastIndexOf(' LIMIT ');
  return end < 0 ? rest : rest.slice(0, end);
}

/** Bind parameters as `-- $1 = 'value'` comment lines under the SQL. */
export function withParams(
  sql: string,
  params: readonly (string | null)[],
): string {
  const values = params.map(
    (value, index) =>
      `-- $${String(index + 1)} = ${value === null ? 'NULL' : JSON.stringify(value)}`,
  );
  return [sql, ...values].join('\n');
}
