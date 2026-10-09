-- SPDX-License-Identifier: Apache-2.0
SELECT o.type_desc, CASE WHEN a.object_id IS NULL THEN N'T-SQL' ELSE N'CLR' END,
    CASE m.execute_as_principal_id WHEN -2 THEN N'OWNER' ELSE USER_NAME(m.execute_as_principal_id) END
FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id
LEFT JOIN sys.sql_modules m ON m.object_id = o.object_id
LEFT JOIN sys.assembly_modules a ON a.object_id = o.object_id
WHERE s.name = @P1 AND o.name = @P2 AND o.type IN ('P','PC','FN','IF','TF','FS','FT')
