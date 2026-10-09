-- SPDX-License-Identifier: Apache-2.0
-- A digest of what the schema tree lists: databases, user schemas, and the
-- relations and routines in them with the row version (`xmin`) of their
-- catalog rows, which every ALTER, RENAME, CREATE or DROP changes.
SELECT pg_catalog.md5(pg_catalog.concat_ws('|',
    (SELECT pg_catalog.string_agg(d.datname, ',' ORDER BY d.datname)
     FROM pg_catalog.pg_database AS d),
    (SELECT pg_catalog.string_agg(n.oid::text || ':' || n.xmin::text, ',' ORDER BY n.oid)
     FROM pg_catalog.pg_namespace AS n),
    (SELECT pg_catalog.string_agg(c.oid::text || ':' || c.xmin::text, ',' ORDER BY c.oid)
     FROM pg_catalog.pg_class AS c
     JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace
     WHERE c.relkind IN ('r', 'p', 'v', 'm', 'f', 'S')
       AND n.nspname NOT LIKE 'pg\_%' AND n.nspname <> 'information_schema'),
    (SELECT pg_catalog.string_agg(p.oid::text || ':' || p.xmin::text, ',' ORDER BY p.oid)
     FROM pg_catalog.pg_proc AS p
     JOIN pg_catalog.pg_namespace AS n ON n.oid = p.pronamespace
     WHERE n.nspname NOT LIKE 'pg\_%' AND n.nspname <> 'information_schema')))
