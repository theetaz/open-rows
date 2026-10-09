-- SPDX-License-Identifier: Apache-2.0
-- Kind, planner estimate, sizes, comment and view query of one relation.
-- `{schema}` and `{name}` are replaced with quoted literals.
-- Columns: relkind, estimate, total bytes, data bytes, index bytes, comment,
-- view definition, partition key.
SELECT c.relkind::text,
       CASE WHEN c.relkind IN ('r', 't', 'm', 'f') AND c.reltuples >= 0
            THEN c.reltuples::bigint END,
       pg_catalog.pg_total_relation_size(c.oid),
       pg_catalog.pg_relation_size(c.oid),
       pg_catalog.pg_indexes_size(c.oid),
       pg_catalog.obj_description(c.oid, 'pg_class'),
       CASE WHEN c.relkind IN ('v', 'm') THEN pg_catalog.pg_get_viewdef(c.oid, true) END,
       CASE WHEN c.relkind = 'p' THEN pg_catalog.pg_get_partkeydef(c.oid) END
FROM pg_catalog.pg_class AS c
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
WHERE n.nspname = {schema}
  AND c.relname = {name}
  AND c.relkind IN ('r', 't', 'p', 'v', 'm', 'f')
