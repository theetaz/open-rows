-- SPDX-License-Identifier: Apache-2.0
-- Explicit ACL entries, including defaults for owners and PUBLIC; membership
-- rows are displayed separately. No role is assumed and no account changes.
SELECT scope::text, object::text, a.privilege_type::text,
    CASE WHEN a.is_grantable THEN 'GRANT WITH GRANT OPTION' ELSE 'GRANT' END::text,
    pg_get_userbyid(a.grantor)::text
FROM (
    SELECT 'database' AS scope, quote_ident(datname) AS object,
        COALESCE(datacl, acldefault('d', datdba)) AS acl FROM pg_database WHERE datname = current_database()
    UNION ALL
    SELECT 'schema', quote_ident(nspname), COALESCE(nspacl, acldefault('n', nspowner)) FROM pg_namespace
    UNION ALL
    SELECT CASE c.relkind WHEN 'S' THEN 'sequence' ELSE 'table' END,
        quote_ident(n.nspname) || '.' || quote_ident(c.relname),
        COALESCE(c.relacl, acldefault(CASE c.relkind WHEN 'S' THEN 'S'::"char" ELSE 'r'::"char" END, c.relowner))
    FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.relkind IN ('r','p','v','m','f','S')
    UNION ALL
    SELECT 'routine', p.oid::regprocedure::text, COALESCE(p.proacl, acldefault('f', p.proowner)) FROM pg_proc p
    UNION ALL
    SELECT 'type', t.oid::regtype::text, COALESCE(t.typacl, acldefault('T', t.typowner)) FROM pg_type t
) objects CROSS JOIN LATERAL aclexplode(objects.acl) a
JOIN pg_roles r ON r.oid = a.grantee
WHERE r.rolname = $1 ORDER BY scope, object, a.privilege_type
