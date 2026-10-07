// SPDX-License-Identifier: Apache-2.0
/** Line count of the S5 benchmark document (NFR-PERF-07). */
export const DOCUMENT_LINES = 5_000;

type Block = (n: number) => string[];

const table: Block = (n) => [
  `-- Orders partition ${String(n)}: generated, deterministic fixture`,
  `CREATE TABLE IF NOT EXISTS sales."Orders_${String(n)}" (`,
  '  id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,',
  '  customer_id uuid NOT NULL DEFAULT gen_random_uuid(),',
  `  total numeric(12, 2) NOT NULL CHECK (total >= ${String(n % 7)}),`,
  "  tags text[] NOT NULL DEFAULT '{}'::text[],",
  "  payload jsonb NOT NULL DEFAULT '{}'::jsonb,",
  '  created_at timestamptz NOT NULL DEFAULT now()',
  ');',
  '',
];

const report: Block = (n) => [
  '/* Weekly revenue with a window function and JSONB filters. */',
  'WITH recent AS (',
  '  SELECT o.customer_id, o.total, o.created_at,',
  "         o.payload ->> 'channel' AS channel",
  `  FROM sales."Orders_${String(n)}" AS o`,
  `  WHERE o.created_at > now() - interval '${String((n % 30) + 1)} days'`,
  `    AND o.payload @> '{"status": "paid"}'::jsonb`,
  ')',
  "SELECT date_trunc('week', created_at) AS week, channel,",
  '       sum(total) OVER (PARTITION BY channel ORDER BY created_at) AS running',
  'FROM recent',
  `WHERE channel ILIKE 'web%' OR channel = ANY (ARRAY['app', 'pos'])`,
  `ORDER BY week DESC NULLS LAST LIMIT ${String(100 + n)};`,
  '',
];

const upsert: Block = (n) => [
  `INSERT INTO sales."Orders_${String(n)}" (customer_id, total, tags, payload)`,
  `VALUES ('00000000-0000-4000-8000-${String(n).padStart(12, '0')}', ${String(n)}.50,`,
  `        ARRAY['bulk', 'q${String(n % 4)}'], jsonb_build_object('note', E'line\\tbreak'))`,
  'ON CONFLICT (id) DO UPDATE',
  '  SET total = EXCLUDED.total, tags = EXCLUDED.tags',
  'RETURNING id, total;',
  '',
];

const fn: Block = (n) => [
  `CREATE OR REPLACE FUNCTION sales.refresh_${String(n)}(p_since timestamptz)`,
  'RETURNS integer LANGUAGE plpgsql AS $$',
  'DECLARE',
  '  affected integer := 0;',
  'BEGIN',
  `  UPDATE sales."Orders_${String(n)}" SET payload = payload || '{"seen": true}'`,
  '  WHERE created_at >= p_since;',
  '  GET DIAGNOSTICS affected = ROW_COUNT;',
  "  RAISE NOTICE 'refreshed % rows', affected;",
  '  RETURN affected;',
  'END;',
  '$$;',
  '',
];

const BLOCKS = [table, report, upsert, fn];

/** A deterministic PostgreSQL script of exactly {@link DOCUMENT_LINES} lines. */
export function generateSqlDocument(lines = DOCUMENT_LINES): string {
  const output: string[] = [];
  for (let n = 0; ; n++) {
    const block = (BLOCKS[n % BLOCKS.length] ?? table)(n);
    if (output.length + block.length > lines) break;
    output.push(...block);
  }
  while (output.length < lines)
    output.push(`SELECT ${String(output.length)} AS filler; -- padding`);
  return output.join('\n');
}
