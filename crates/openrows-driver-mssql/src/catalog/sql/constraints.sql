-- CHECK constraints and foreign keys of @P1.@P2; foreign keys list their
-- column pairs in key order. Primary and unique keys come from the indexes.
SELECT N'C' AS kind, cc.name, cc.definition, COL_NAME(cc.parent_object_id, cc.parent_column_id),
    NULL, NULL, NULL, 0
FROM sys.check_constraints AS cc
JOIN sys.objects AS o ON o.object_id = cc.parent_object_id
JOIN sys.schemas AS s ON s.schema_id = o.schema_id
WHERE s.name = @P1 AND o.name = @P2
UNION ALL
SELECT N'F', fk.name, NULL, COL_NAME(fkc.parent_object_id, fkc.parent_column_id),
    OBJECT_SCHEMA_NAME(fk.referenced_object_id), OBJECT_NAME(fk.referenced_object_id),
    COL_NAME(fkc.referenced_object_id, fkc.referenced_column_id), fkc.constraint_column_id
FROM sys.foreign_keys AS fk
JOIN sys.foreign_key_columns AS fkc ON fkc.constraint_object_id = fk.object_id
JOIN sys.objects AS o ON o.object_id = fk.parent_object_id
JOIN sys.schemas AS s ON s.schema_id = o.schema_id
WHERE s.name = @P1 AND o.name = @P2
ORDER BY 1, 2, 8
