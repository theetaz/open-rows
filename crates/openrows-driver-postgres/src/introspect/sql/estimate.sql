-- SPDX-License-Identifier: Apache-2.0
-- Planner row estimate of one relation; NULL for views and for tables that
-- were never analysed. `{schema}` and `{name}` are replaced with quoted literals.
SELECT CASE WHEN c.relkind IN ('r', 'm', 'f') AND c.reltuples >= 0
            THEN c.reltuples::bigint END
FROM pg_catalog.pg_class AS c
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
WHERE n.nspname = {schema}
  AND c.relname = {name}
  AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
