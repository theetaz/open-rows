-- SPDX-License-Identifier: Apache-2.0
-- pg_roles exposes public attributes; never select rolpassword.
SELECT rolname::text, rolcanlogin::text, rolsuper::text, rolcreatedb::text,
    rolcreaterole::text, rolinherit::text, rolreplication::text, rolbypassrls::text,
    rolconnlimit::text, rolvaliduntil::text
FROM pg_roles ORDER BY rolname
