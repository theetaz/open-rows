-- SPDX-License-Identifier: Apache-2.0
SELECT CASE WHEN t.is_table_type = 1 THEN N'table' WHEN t.is_assembly_type = 1 THEN N'CLR' ELSE N'alias' END,
    t.name, TYPE_NAME(t.system_type_id), t.max_length, t.precision, t.scale, t.is_nullable,
    t.collation_name
FROM sys.types t JOIN sys.schemas s ON s.schema_id = t.schema_id
WHERE t.is_user_defined = 1 AND s.name = @P1 AND t.name = @P2
