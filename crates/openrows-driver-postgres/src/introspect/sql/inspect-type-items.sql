-- SPDX-License-Identifier: Apache-2.0
SELECT section, name, value FROM (
SELECT 'Enum labels'::text AS section, e.enumlabel::text AS name, NULL::text AS value,
    e.enumsortorder::numeric AS ordinal
FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace
JOIN pg_enum e ON e.enumtypid = t.oid WHERE n.nspname = $1 AND t.typname = $2
UNION ALL
SELECT 'Composite fields', a.attname::text, format_type(a.atttypid, a.atttypmod)::text, a.attnum
FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace
JOIN pg_attribute a ON a.attrelid = t.typrelid
WHERE n.nspname = $1 AND t.typname = $2 AND a.attnum > 0 AND NOT a.attisdropped
UNION ALL
SELECT 'Domain constraints', c.conname::text, pg_get_constraintdef(c.oid, true)::text, c.oid::bigint::numeric
FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace
JOIN pg_constraint c ON c.contypid = t.oid WHERE n.nspname = $1 AND t.typname = $2
 ) AS items ORDER BY section, ordinal
