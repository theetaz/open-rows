-- SPDX-License-Identifier: Apache-2.0
-- User (non-internal) triggers of one relation, ordered by name. `{schema}`
-- and `{name}` are replaced with quoted literals.
-- Columns: name, tgtype bit mask, tgenabled letter, function, definition.
SELECT t.tgname,
       t.tgtype::integer,
       t.tgenabled::text,
       pg_catalog.quote_ident(pn.nspname) || '.' || pg_catalog.quote_ident(p.proname) || '()',
       pg_catalog.pg_get_triggerdef(t.oid, true)
FROM pg_catalog.pg_trigger AS t
JOIN pg_catalog.pg_class AS c ON c.oid = t.tgrelid
JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
JOIN pg_catalog.pg_proc AS p ON p.oid = t.tgfoid
JOIN pg_catalog.pg_namespace AS pn ON pn.oid = p.pronamespace
WHERE n.nspname = {schema}
  AND c.relname = {name}
  AND NOT t.tgisinternal
ORDER BY t.tgname
