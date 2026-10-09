-- SPDX-License-Identifier: Apache-2.0
SELECT name, N'server', type_desc, is_disabled, default_database_name,
    CONVERT(nvarchar(max), LOGINPROPERTY(name, 'IsExpired'))
FROM sys.server_principals WHERE type IN ('S','U','G','R','E','X')
UNION ALL
SELECT name, N'database', type_desc, NULL, default_schema_name, NULL
FROM sys.database_principals WHERE type IN ('S','U','G','R','A','E','X')
ORDER BY 2, 1
