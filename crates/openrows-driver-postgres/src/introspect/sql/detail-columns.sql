-- SPDX-License-Identifier: Apache-2.0
-- Columns of one table-like relation in ordinal order, for the Structure
-- grid and the DDL. `{schema}` and `{name}` are replaced with quoted literals.
-- Columns: name, formatted type, nullable, default or generation expression,
-- generated kind ('a' identity always, 'd' identity by default, 's' stored,
-- '' none), comment.
SELECT a.attname,
       pg_catalog.format_type(a.atttypid, a.atttypmod),
       NOT a.attnotnull,
       pg_catalog.pg_get_expr(d.adbin, d.adrelid),
       CASE WHEN a.attidentity IN ('a', 'd') THEN a.attidentity::text
            WHEN a.attgenerated = 's' THEN 's'
            ELSE '' END,
       pg_catalog.col_description(c.oid, a.attnum)
FROM pg_catalog.pg_attribute AS a
JOIN pg_catalog.pg_class AS c ON c.oid = a.attrelid
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
LEFT JOIN pg_catalog.pg_attrdef AS d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
WHERE n.nspname = {schema}
  AND c.relname = {name}
  AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
  AND a.attnum > 0
  AND NOT a.attisdropped
ORDER BY a.attnum
