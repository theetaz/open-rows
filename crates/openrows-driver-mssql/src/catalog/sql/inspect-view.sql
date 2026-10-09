-- SPDX-License-Identifier: Apache-2.0
SELECT m.definition FROM sys.views v JOIN sys.schemas s ON s.schema_id = v.schema_id
LEFT JOIN sys.sql_modules m ON m.object_id = v.object_id WHERE s.name = @P1 AND v.name = @P2
