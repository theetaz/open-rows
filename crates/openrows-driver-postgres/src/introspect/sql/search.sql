-- SPDX-License-Identifier: Apache-2.0
-- Bounded name index of tables, views, sequences and routines in every
-- non-system schema, for quick open and autocomplete. Comments are omitted.
-- Columns: schema, name, kind, routine arguments, row estimate, comment.
SELECT o.*
FROM (
    SELECT n.nspname,
           c.relname,
           CASE c.relkind
               WHEN 'r' THEN 'table'
               WHEN 'p' THEN 'table'
               WHEN 'v' THEN 'view'
               WHEN 'm' THEN 'materialized_view'
               WHEN 'f' THEN 'foreign_table'
               ELSE 'sequence'
           END,
           NULL::text,
           CASE WHEN c.relkind IN ('r', 'm') AND c.reltuples >= 0
                THEN c.reltuples::bigint END,
           NULL::text
    FROM pg_catalog.pg_class AS c
    JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
    WHERE c.relkind IN ('r', 'p', 'v', 'm', 'f', 'S')
      AND NOT c.relispartition
      AND n.nspname !~ '^pg_'
      AND n.nspname <> 'information_schema'
    UNION ALL
    SELECT n.nspname,
           p.proname,
           CASE p.prokind WHEN 'p' THEN 'procedure' ELSE 'function' END,
           pg_catalog.pg_get_function_identity_arguments(p.oid),
           NULL::bigint,
           NULL::text
    FROM pg_catalog.pg_proc AS p
    JOIN pg_catalog.pg_namespace AS n ON n.oid = p.pronamespace
    WHERE p.prokind IN ('f', 'p', 'w')
      AND n.nspname !~ '^pg_'
      AND n.nspname <> 'information_schema'
      AND NOT EXISTS (
          SELECT 1 FROM pg_catalog.pg_depend AS d
          WHERE d.classid = 'pg_catalog.pg_proc'::pg_catalog.regclass
            AND d.objid = p.oid
            AND d.deptype = 'e'
      )
) AS o (schema, name, kind, arguments, row_estimate, comment)
ORDER BY o.schema, o.name, o.arguments
LIMIT 20000
