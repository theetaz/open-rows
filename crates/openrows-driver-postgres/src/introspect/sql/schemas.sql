-- SPDX-License-Identifier: Apache-2.0
-- Schemas of the current database; temporary schemas are skipped; TOAST stays available to the visibility toggle.
-- Columns: name, system flag, comment.
SELECT n.nspname,
       (n.nspname IN ('pg_catalog', 'information_schema') OR n.nspname LIKE 'pg_toast%'),
       pg_catalog.obj_description(n.oid, 'pg_namespace')
FROM pg_catalog.pg_namespace AS n
WHERE n.nspname !~ '^pg_(temp_|toast_temp_)'
ORDER BY 2, n.nspname
