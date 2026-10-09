-- Foreign-key column pairs declared in @P1, in key order.
SELECT OBJECT_NAME(fk.parent_object_id), fk.name,
    COL_NAME(fkc.parent_object_id, fkc.parent_column_id),
    OBJECT_SCHEMA_NAME(fk.referenced_object_id), OBJECT_NAME(fk.referenced_object_id),
    COL_NAME(fkc.referenced_object_id, fkc.referenced_column_id)
FROM sys.foreign_keys AS fk
JOIN sys.foreign_key_columns AS fkc ON fkc.constraint_object_id = fk.object_id
WHERE fk.schema_id = SCHEMA_ID(@P1)
ORDER BY 1, 2, fkc.constraint_column_id
