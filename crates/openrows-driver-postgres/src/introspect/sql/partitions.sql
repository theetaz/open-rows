-- SPDX-License-Identifier: Apache-2.0
-- Direct partitions of one partitioned table, ordered by name. `{schema}`
-- and `{name}` are replaced with quoted literals.
-- Columns: schema, name, bound, sub-partitioned flag, estimate.
SELECT pn.nspname,
       pc.relname,
       pg_catalog.pg_get_expr(pc.relpartbound, pc.oid),
       pc.relkind = 'p',
       CASE WHEN pc.reltuples >= 0 THEN pc.reltuples::bigint END
FROM pg_catalog.pg_inherits AS i
JOIN pg_catalog.pg_class AS c ON c.oid = i.inhparent
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
JOIN pg_catalog.pg_class AS pc ON pc.oid = i.inhrelid
JOIN pg_catalog.pg_namespace AS pn ON pn.oid = pc.relnamespace
WHERE n.nspname = {schema}
  AND c.relname = {name}
  AND pc.relispartition
ORDER BY pc.relname
