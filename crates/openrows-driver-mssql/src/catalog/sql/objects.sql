-- Objects of one schema (@P1) or, without a schema, of every user schema;
-- `{top}` limits the search index. Row counts come from sys.partitions,
-- which needs no extra permission.
SELECT {top} * FROM (
SELECT s.name AS schema_name, o.name, o.type,
    (SELECT SUM(p.rows) FROM sys.partitions AS p
        WHERE p.object_id = o.object_id AND p.index_id IN (0, 1)),
    CAST(ep.value AS nvarchar(max))
FROM sys.objects AS o
JOIN sys.schemas AS s ON s.schema_id = o.schema_id
LEFT JOIN sys.extended_properties AS ep
    ON ep.class = 1 AND ep.major_id = o.object_id AND ep.minor_id = 0
    AND ep.name = N'MS_Description'
WHERE o.type IN ('U', 'V', 'P', 'PC', 'FN', 'IF', 'TF', 'FS', 'FT', 'SO')
    AND o.is_ms_shipped = 0 {schema}
UNION ALL
SELECT s.name, t.name, 'TY', NULL, CAST(ep.value AS nvarchar(max))
FROM sys.types t JOIN sys.schemas s ON s.schema_id = t.schema_id
LEFT JOIN sys.extended_properties ep ON ep.class = 6 AND ep.major_id = t.user_type_id
    AND ep.minor_id = 0 AND ep.name = N'MS_Description'
WHERE t.is_user_defined = 1 {schema}
) objects (schema_name, name, type, row_estimate, comment) ORDER BY name, schema_name
