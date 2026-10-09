-- SPDX-License-Identifier: Apache-2.0
-- Columns of the first `{limit}` tables of one schema for the ER diagram, in
-- table-name then ordinal order. Partitions are skipped. A table without
-- columns yields one row with NULL column fields. `{schema}` is replaced with
-- a quoted literal and `{limit}` with a number.
-- Columns: table, column, formatted type, nullable, primary-key member.
WITH t AS (
    SELECT c.oid, c.relname
    FROM pg_catalog.pg_class AS c
    JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
    WHERE n.nspname = {schema}
      AND c.relkind IN ('r', 'p')
      AND NOT c.relispartition
    ORDER BY c.relname
    LIMIT {limit}
)
SELECT t.relname,
       a.attname,
       pg_catalog.format_type(a.atttypid, a.atttypmod),
       NOT a.attnotnull,
       EXISTS (SELECT 1
               FROM pg_catalog.pg_index AS x
               WHERE x.indrelid = t.oid
                 AND x.indisprimary
                 AND a.attnum = ANY (x.indkey))
FROM t
LEFT JOIN pg_catalog.pg_attribute AS a
       ON a.attrelid = t.oid
      AND a.attnum > 0
      AND NOT a.attisdropped
ORDER BY t.relname, a.attnum
