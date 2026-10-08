-- SPDX-License-Identifier: Apache-2.0
-- Constraints of one table: primary key, unique, check, foreign key and
-- exclusion, in that order then by name. `{schema}` and `{name}` are replaced
-- with quoted literals. Column lists are `quote_ident` names joined by commas.
-- Columns: name, type letter, definition, columns, referenced schema,
-- referenced table, referenced columns.
SELECT k.conname,
       k.contype::text,
       pg_catalog.pg_get_constraintdef(k.oid, true),
       pg_catalog.array_to_string(ARRAY(
           SELECT pg_catalog.quote_ident(a.attname)
           FROM pg_catalog.unnest(k.conkey) WITH ORDINALITY AS u(num, ord)
           JOIN pg_catalog.pg_attribute AS a ON a.attrelid = k.conrelid AND a.attnum = u.num
           ORDER BY u.ord), ','),
       fn.nspname,
       fc.relname,
       pg_catalog.array_to_string(ARRAY(
           SELECT pg_catalog.quote_ident(a.attname)
           FROM pg_catalog.unnest(k.confkey) WITH ORDINALITY AS u(num, ord)
           JOIN pg_catalog.pg_attribute AS a ON a.attrelid = k.confrelid AND a.attnum = u.num
           ORDER BY u.ord), ',')
FROM pg_catalog.pg_constraint AS k
JOIN pg_catalog.pg_class AS c ON c.oid = k.conrelid
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
LEFT JOIN pg_catalog.pg_class AS fc ON fc.oid = k.confrelid
LEFT JOIN pg_catalog.pg_namespace AS fn ON fn.oid = fc.relnamespace
WHERE n.nspname = {schema}
  AND c.relname = {name}
  AND k.contype IN ('p', 'u', 'c', 'f', 'x')
ORDER BY CASE k.contype WHEN 'p' THEN 0 WHEN 'u' THEN 1 WHEN 'c' THEN 2 WHEN 'f' THEN 3 ELSE 4 END,
         k.conname
