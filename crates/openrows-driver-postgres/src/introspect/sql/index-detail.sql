-- SPDX-License-Identifier: Apache-2.0
-- Indexes of one table or materialized view with their full definitions,
-- ordered by name. `{schema}` and `{name}` are replaced with quoted literals.
-- Columns: name, access method, unique, primary, backs a constraint, key
-- columns or expressions, INCLUDE columns (`quote_ident` names joined by
-- commas), predicate, comment, definition.
SELECT i.relname,
       am.amname,
       x.indisunique,
       x.indisprimary,
       EXISTS (SELECT 1 FROM pg_catalog.pg_constraint AS k
               WHERE k.conindid = x.indexrelid AND k.conrelid = x.indrelid
                 AND k.contype IN ('p', 'u', 'x')),
       (SELECT pg_catalog.string_agg(pg_catalog.pg_get_indexdef(x.indexrelid, k, true), ', ' ORDER BY k)
        FROM pg_catalog.generate_series(1, x.indnkeyatts) AS k),
       pg_catalog.array_to_string(ARRAY(
           SELECT pg_catalog.quote_ident(a.attname)
           FROM pg_catalog.unnest(x.indkey::pg_catalog.int2[]) WITH ORDINALITY AS u(num, ord)
           JOIN pg_catalog.pg_attribute AS a ON a.attrelid = x.indrelid AND a.attnum = u.num
           WHERE u.ord > x.indnkeyatts
           ORDER BY u.ord), ','),
       pg_catalog.pg_get_expr(x.indpred, x.indrelid, true),
       pg_catalog.obj_description(i.oid, 'pg_class'),
       pg_catalog.pg_get_indexdef(x.indexrelid)
FROM pg_catalog.pg_index AS x
JOIN pg_catalog.pg_class AS i ON i.oid = x.indexrelid
JOIN pg_catalog.pg_class AS c ON c.oid = x.indrelid
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
JOIN pg_catalog.pg_am AS am ON am.oid = i.relam
WHERE n.nspname = {schema}
  AND c.relname = {name}
ORDER BY i.relname
