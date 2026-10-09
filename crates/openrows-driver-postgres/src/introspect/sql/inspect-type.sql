-- SPDX-License-Identifier: Apache-2.0
SELECT CASE t.typtype WHEN 'e' THEN 'enum' WHEN 'c' THEN 'composite'
    WHEN 'd' THEN 'domain' WHEN 'r' THEN 'range' END::text,
    CASE WHEN t.typtype = 'd' THEN format_type(t.typbasetype, t.typtypmod) END::text,
    t.typnotnull::text, t.typdefault::text,
    CASE WHEN r.rngsubtype IS NOT NULL THEN format_type(r.rngsubtype, NULL) END::text,
    obj_description(t.oid, 'pg_type')::text
FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace
LEFT JOIN pg_range r ON r.rngtypid = t.oid
WHERE n.nspname = $1 AND t.typname = $2
    AND t.typtype IN ('e', 'c', 'd', 'r')
