-- Kind, row estimate, sizes (used 8 KiB pages: all, then the heap or
-- clustered index), comment and module definition of @P1.@P2.
SELECT o.type,
    (SELECT SUM(p.rows) FROM sys.partitions AS p
        WHERE p.object_id = o.object_id AND p.index_id IN (0, 1)),
    (SELECT SUM(a.used_pages) FROM sys.partitions AS p
        JOIN sys.allocation_units AS a ON a.container_id = p.partition_id
        WHERE p.object_id = o.object_id) * 8192,
    (SELECT SUM(a.used_pages) FROM sys.partitions AS p
        JOIN sys.allocation_units AS a ON a.container_id = p.partition_id
        WHERE p.object_id = o.object_id AND p.index_id IN (0, 1)) * 8192,
    CAST(ep.value AS nvarchar(max)), OBJECT_DEFINITION(o.object_id)
FROM sys.objects AS o
JOIN sys.schemas AS s ON s.schema_id = o.schema_id
LEFT JOIN sys.extended_properties AS ep
    ON ep.class = 1 AND ep.major_id = o.object_id AND ep.minor_id = 0
    AND ep.name = N'MS_Description'
WHERE s.name = @P1 AND o.name = @P2 AND o.type IN ('U', 'V')
