-- Columns of @P1.@P2 in ordinal order, with what the structure views, the
-- editor and the DDL need: the declared type parts, nullability, default
-- (and its constraint name), comment, primary-key membership, identity,
-- computed definition and collation.
SELECT c.name, TYPE_NAME(c.user_type_id), TYPE_NAME(c.system_type_id),
    c.max_length, c.precision, c.scale, c.is_nullable,
    OBJECT_DEFINITION(c.default_object_id), dc.name,
    CAST(ep.value AS nvarchar(max)),
    CASE WHEN EXISTS (
        SELECT 1 FROM sys.indexes AS i
        JOIN sys.index_columns AS ic
            ON ic.object_id = i.object_id AND ic.index_id = i.index_id
        WHERE i.object_id = c.object_id AND i.is_primary_key = 1
            AND ic.column_id = c.column_id) THEN 1 ELSE 0 END,
    c.is_identity, cc.definition, c.collation_name,
    CONVERT(nvarchar(40), IDENT_SEED(QUOTENAME(s.name) + N'.' + QUOTENAME(o.name))),
    CONVERT(nvarchar(40), IDENT_INCR(QUOTENAME(s.name) + N'.' + QUOTENAME(o.name)))
FROM sys.columns AS c
JOIN sys.objects AS o ON o.object_id = c.object_id
JOIN sys.schemas AS s ON s.schema_id = o.schema_id
LEFT JOIN sys.default_constraints AS dc ON dc.object_id = c.default_object_id
LEFT JOIN sys.computed_columns AS cc
    ON cc.object_id = c.object_id AND cc.column_id = c.column_id
LEFT JOIN sys.extended_properties AS ep
    ON ep.class = 1 AND ep.major_id = c.object_id AND ep.minor_id = c.column_id
    AND ep.name = N'MS_Description'
WHERE s.name = @P1 AND o.name = @P2
ORDER BY c.column_id
