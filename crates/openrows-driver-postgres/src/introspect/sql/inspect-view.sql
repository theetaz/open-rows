-- SPDX-License-Identifier: Apache-2.0
SELECT c.relkind::text, c.relispopulated::text, pg_get_viewdef(c.oid, true)::text,
    ('CREATE ' || CASE c.relkind WHEN 'm' THEN 'MATERIALIZED ' ELSE '' END ||
    'VIEW ' || quote_ident(n.nspname) || '.' || quote_ident(c.relname) || ' AS ' ||
    rtrim(pg_get_viewdef(c.oid, true), E'; \n\t') || CASE WHEN c.relkind = 'm' AND NOT c.relispopulated
    THEN ' WITH NO DATA' ELSE '' END || ';')::text
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = $1 AND c.relname = $2 AND c.relkind IN ('v', 'm')
