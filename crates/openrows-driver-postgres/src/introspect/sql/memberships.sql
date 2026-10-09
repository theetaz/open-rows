-- SPDX-License-Identifier: Apache-2.0
SELECT member.rolname::text, parent.rolname::text FROM pg_auth_members m
JOIN pg_roles member ON member.oid = m.member
JOIN pg_roles parent ON parent.oid = m.roleid ORDER BY member.rolname, parent.rolname
