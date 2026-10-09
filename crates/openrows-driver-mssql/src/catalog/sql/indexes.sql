-- Indexes of @P1.@P2 with their key columns in key order, then the
-- included columns; heaps (type 0) are not indexes.
SELECT i.name, i.is_unique, i.is_primary_key, i.is_unique_constraint,
    LOWER(i.type_desc), i.filter_definition, col.name,
    ic.is_included_column, ic.is_descending_key
FROM sys.indexes AS i
JOIN sys.objects AS o ON o.object_id = i.object_id
JOIN sys.schemas AS s ON s.schema_id = o.schema_id
JOIN sys.index_columns AS ic
    ON ic.object_id = i.object_id AND ic.index_id = i.index_id
JOIN sys.columns AS col
    ON col.object_id = ic.object_id AND col.column_id = ic.column_id
WHERE s.name = @P1 AND o.name = @P2 AND i.type > 0
ORDER BY i.name, ic.is_included_column, ic.key_ordinal, ic.index_column_id
