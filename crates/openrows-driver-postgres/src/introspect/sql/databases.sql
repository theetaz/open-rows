-- SPDX-License-Identifier: Apache-2.0
-- Databases a client can connect to; templates and closed databases are skipped.
-- Columns: name, system flag, comment.
SELECT d.datname,
       d.datname IN ('postgres', 'rdsadmin', 'azure_maintenance', 'azure_sys',
                     'cloudsqladmin'),
       pg_catalog.shobj_description(d.oid, 'pg_database')
FROM pg_catalog.pg_database AS d
WHERE NOT d.datistemplate
  AND d.datallowconn
ORDER BY d.datname
