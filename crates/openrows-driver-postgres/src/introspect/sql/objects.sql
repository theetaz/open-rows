-- SPDX-License-Identifier: Apache-2.0
-- Objects of one schema, filtered by kind. Partitions and extension members
-- are skipped. `$1` and `{kinds}` are replaced with quoted literals.
-- Columns: schema, name, kind, routine arguments, row estimate, comment.
SELECT o.schema::text, o.name::text, o.kind::text, o.arguments::text, o.row_estimate::text, o.comment::text
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
           pg_catalog.obj_description(c.oid, 'pg_class')
    FROM pg_catalog.pg_class AS c
    JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
    WHERE n.nspname = $1
      AND c.relkind IN ('r', 'p', 'v', 'm', 'f', 'S')
      AND NOT c.relispartition
    UNION ALL
    SELECT n.nspname,
           p.proname,
           CASE p.prokind WHEN 'p' THEN 'procedure' ELSE 'function' END,
           pg_catalog.pg_get_function_identity_arguments(p.oid),
           NULL::bigint,
           pg_catalog.obj_description(p.oid, 'pg_proc')
    FROM pg_catalog.pg_proc AS p
    JOIN pg_catalog.pg_namespace AS n ON n.oid = p.pronamespace
    WHERE n.nspname = $1
      AND p.prokind IN ('f', 'p', 'w')
      AND NOT EXISTS (
          SELECT 1 FROM pg_catalog.pg_depend AS d
          WHERE d.classid = 'pg_catalog.pg_proc'::pg_catalog.regclass
            AND d.objid = p.oid
            AND d.deptype = 'e'
      )
    UNION ALL
    SELECT n.nspname, t.typname, 'type', NULL::text, NULL::bigint,
           pg_catalog.obj_description(t.oid, 'pg_type')
    FROM pg_catalog.pg_type t
    JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
    LEFT JOIN pg_catalog.pg_class c ON c.oid = t.typrelid
    WHERE n.nspname = $1
      AND (t.typtype IN ('e', 'd', 'r') OR (t.typtype = 'c' AND c.relkind = 'c'))
      AND NOT EXISTS (SELECT 1 FROM pg_catalog.pg_depend d
          WHERE d.classid = 'pg_catalog.pg_type'::regclass AND d.objid = t.oid AND d.deptype = 'e')
) AS o (schema, name, kind, arguments, row_estimate, comment)
WHERE o.kind = ANY(string_to_array($2, ','))
ORDER BY o.name, o.arguments
