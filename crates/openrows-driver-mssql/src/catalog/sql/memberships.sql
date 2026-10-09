-- SPDX-License-Identifier: Apache-2.0
SELECT member.name, N'server', parent.name FROM sys.server_role_members m
JOIN sys.server_principals member ON member.principal_id = m.member_principal_id
JOIN sys.server_principals parent ON parent.principal_id = m.role_principal_id
UNION ALL
SELECT member.name, N'database', parent.name FROM sys.database_role_members m
JOIN sys.database_principals member ON member.principal_id = m.member_principal_id
JOIN sys.database_principals parent ON parent.principal_id = m.role_principal_id
ORDER BY 2, 1, 3
