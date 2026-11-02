-- SPDX-License-Identifier: Apache-2.0
-- `table-info.sql` for CockroachDB, which has no relation size functions and
-- no partition key text: sizes read as zero.
SELECT c.relkind::text,
       CASE WHEN c.relkind IN ('r', 't', 'm', 'f') AND c.reltuples >= 0
            THEN c.reltuples::bigint END,
       0,
       0,
       0,
       pg_catalog.obj_description(c.oid, 'pg_class'),
       CASE WHEN c.relkind IN ('v', 'm') THEN pg_catalog.pg_get_viewdef(c.oid, true) END,
       NULL::text
FROM pg_catalog.pg_class AS c
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
WHERE n.nspname = {schema}
  AND c.relname = {name}
  AND c.relkind IN ('r', 't', 'p', 'v', 'm', 'f')
