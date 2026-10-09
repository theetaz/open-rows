-- SPDX-License-Identifier: Apache-2.0
SELECT p.class_desc,
    CASE p.class WHEN 0 THEN QUOTENAME(DB_NAME())
        WHEN 1 THEN QUOTENAME(OBJECT_SCHEMA_NAME(p.major_id)) + N'.' + QUOTENAME(OBJECT_NAME(p.major_id))
            + CASE WHEN p.minor_id > 0 THEN N'.' + QUOTENAME(COL_NAME(p.major_id, p.minor_id)) ELSE N'' END
        WHEN 3 THEN QUOTENAME(SCHEMA_NAME(p.major_id))
        WHEN 4 THEN QUOTENAME(USER_NAME(p.major_id))
        WHEN 6 THEN QUOTENAME(TYPE_NAME(p.major_id))
        ELSE CONVERT(nvarchar(max), p.major_id) END,
    p.permission_name, p.state_desc, USER_NAME(p.grantor_principal_id)
FROM sys.database_permissions p JOIN sys.database_principals r ON r.principal_id = p.grantee_principal_id
WHERE r.name = @P1 AND @P2 = N'database'
UNION ALL
SELECT p.class_desc, CASE p.class WHEN 100 THEN N'SERVER'
    WHEN 101 THEN (SELECT QUOTENAME(name) FROM sys.server_principals WHERE principal_id = p.major_id)
    WHEN 105 THEN (SELECT QUOTENAME(name) FROM sys.endpoints WHERE endpoint_id = p.major_id)
    ELSE CONVERT(nvarchar(max), p.major_id) END,
    p.permission_name, p.state_desc, SUSER_NAME(p.grantor_principal_id)
FROM sys.server_permissions p JOIN sys.server_principals r ON r.principal_id = p.grantee_principal_id
WHERE r.name = @P1 AND @P2 = N'server'
ORDER BY 1, 2, 3
