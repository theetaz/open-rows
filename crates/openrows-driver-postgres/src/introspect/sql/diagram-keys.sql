-- SPDX-License-Identifier: Apache-2.0
-- Foreign keys declared on tables of one schema for the ER diagram, one row
-- per column pair, in table, constraint and key order. `{schema}` is
-- replaced with a quoted literal.
-- Columns: table, constraint, column, referenced schema, referenced table,
-- referenced column.
SELECT c.relname,
       k.conname,
       a.attname,
       fn.nspname,
       fc.relname,
       fa.attname
FROM pg_catalog.pg_constraint AS k
JOIN pg_catalog.pg_class AS c ON c.oid = k.conrelid
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
JOIN pg_catalog.pg_class AS fc ON fc.oid = k.confrelid
JOIN pg_catalog.pg_namespace AS fn ON fn.oid = fc.relnamespace
CROSS JOIN LATERAL ROWS FROM (pg_catalog.unnest(k.conkey), pg_catalog.unnest(k.confkey))
    WITH ORDINALITY AS u(src, dst, ord)
JOIN pg_catalog.pg_attribute AS a ON a.attrelid = k.conrelid AND a.attnum = u.src
JOIN pg_catalog.pg_attribute AS fa ON fa.attrelid = k.confrelid AND fa.attnum = u.dst
WHERE n.nspname = {schema}
  AND k.contype = 'f'
ORDER BY c.relname, k.conname, u.ord
