-- DML triggers of @P1.@P2 with their events and definition.
SELECT t.name, t.is_instead_of_trigger, t.is_disabled,
    STUFF((SELECT N',' + te.type_desc FROM sys.trigger_events AS te
        WHERE te.object_id = t.object_id FOR XML PATH('')), 1, 1, N''),
    OBJECT_DEFINITION(t.object_id)
FROM sys.triggers AS t
JOIN sys.objects AS o ON o.object_id = t.parent_id
JOIN sys.schemas AS s ON s.schema_id = o.schema_id
WHERE s.name = @P1 AND o.name = @P2
ORDER BY t.name
