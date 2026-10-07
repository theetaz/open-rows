-- SPDX-License-Identifier: Apache-2.0
-- Only simple, valid, non-partial unique keys whose non-null values are lossless
-- in the grid's display codec qualify for editing. Prefer the primary key.
SELECT a.attname
FROM pg_catalog.pg_index i
JOIN pg_catalog.pg_class c ON c.oid = i.indrelid
JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
JOIN LATERAL unnest(i.indkey) WITH ORDINALITY k(attnum, position) ON k.position <= i.indnkeyatts
JOIN pg_catalog.pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.attnum
WHERE n.nspname = {schema} AND c.relname = {name} AND c.relkind IN ('r', 'p')
  AND i.indexrelid = (
    SELECT candidate.indexrelid FROM pg_catalog.pg_index candidate
    WHERE candidate.indrelid = c.oid AND candidate.indisunique AND candidate.indisvalid
      AND candidate.indpred IS NULL AND candidate.indexprs IS NULL
      AND NOT EXISTS (
        SELECT 1 FROM unnest(candidate.indkey) WITH ORDINALITY ck(attnum, position)
        LEFT JOIN pg_catalog.pg_attribute ca ON ca.attrelid = c.oid AND ca.attnum = ck.attnum
        WHERE ck.position <= candidate.indnkeyatts
          AND (ca.attnum IS NULL OR NOT ca.attnotnull OR ca.atttypid NOT IN (16,20,21,23,25,1042,1043,2950))
      )
    ORDER BY candidate.indisprimary DESC, candidate.indexrelid LIMIT 1
  )
ORDER BY k.position
