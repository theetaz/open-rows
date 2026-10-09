-- SPDX-License-Identifier: Apache-2.0
SELECT p.name, TYPE_NAME(p.user_type_id), TYPE_NAME(p.system_type_id), p.max_length,
    p.precision, p.scale, CASE WHEN p.is_output = 1 THEN N'OUT' ELSE N'IN' END,
    p.is_readonly, p.parameter_id
FROM sys.parameters p JOIN sys.objects o ON o.object_id = p.object_id
JOIN sys.schemas s ON s.schema_id = o.schema_id WHERE s.name = @P1 AND o.name = @P2
ORDER BY p.parameter_id
