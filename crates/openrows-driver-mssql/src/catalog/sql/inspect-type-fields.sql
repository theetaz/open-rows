-- SPDX-License-Identifier: Apache-2.0
SELECT c.name, TYPE_NAME(c.user_type_id), TYPE_NAME(c.system_type_id), c.max_length,
    c.precision, c.scale, c.is_nullable
FROM sys.table_types t JOIN sys.schemas s ON s.schema_id = t.schema_id
JOIN sys.columns c ON c.object_id = t.type_table_object_id
WHERE s.name = @P1 AND t.name = @P2 ORDER BY c.column_id
