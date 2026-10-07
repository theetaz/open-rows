-- SPDX-License-Identifier: Apache-2.0
-- Indexes of one table or materialized view, ordered by name. `{schema}`
-- and `{name}` are replaced with quoted literals. Key columns exclude
-- INCLUDE columns and show expressions as written.
-- Columns: name, key columns, unique, primary, access method, predicate.
SELECT i.relname,
       (SELECT pg_catalog.string_agg(pg_catalog.pg_get_indexdef(x.indexrelid, k, true), ', ' ORDER BY k)
        FROM pg_catalog.generate_series(1, x.indnkeyatts) AS k),
       x.indisunique,
       x.indisprimary,
       am.amname,
       pg_catalog.pg_get_expr(x.indpred, x.indrelid, true)
FROM pg_catalog.pg_index AS x
JOIN pg_catalog.pg_class AS i ON i.oid = x.indexrelid
JOIN pg_catalog.pg_class AS c ON c.oid = x.indrelid
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
JOIN pg_catalog.pg_am AS am ON am.oid = i.relam
WHERE n.nspname = {schema}
  AND c.relname = {name}
ORDER BY i.relname
