-- Columns of the first {limit} tables of @P1 by name (tables without
-- columns still listed), for the ER diagram.
SELECT t.name, c.name, TYPE_NAME(c.user_type_id), TYPE_NAME(c.system_type_id),
    c.max_length, c.precision, c.scale, c.is_nullable,
    CASE WHEN EXISTS (
        SELECT 1 FROM sys.indexes AS i
        JOIN sys.index_columns AS ic
            ON ic.object_id = i.object_id AND ic.index_id = i.index_id
        WHERE i.object_id = c.object_id AND i.is_primary_key = 1
            AND ic.column_id = c.column_id) THEN 1 ELSE 0 END
FROM (SELECT TOP ({limit}) tb.object_id, tb.name FROM sys.tables AS tb
        JOIN sys.schemas AS s ON s.schema_id = tb.schema_id
        WHERE s.name = @P1 AND tb.is_ms_shipped = 0 ORDER BY tb.name) AS t
LEFT JOIN sys.columns AS c ON c.object_id = t.object_id
ORDER BY t.name, c.column_id
