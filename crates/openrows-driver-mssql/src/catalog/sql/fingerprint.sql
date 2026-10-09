-- An order-independent digest of the schemas and of every user object with
-- its modification time (ALTER changes it).
SELECT CONCAT((SELECT COUNT(*) FROM sys.schemas), N':', COUNT(*), N':',
    CHECKSUM_AGG(CHECKSUM(o.object_id, o.name, o.type, o.schema_id, o.modify_date)), N':',
    (SELECT CHECKSUM_AGG(CHECKSUM(user_type_id, name, schema_id, max_length, precision, scale, is_nullable))
     FROM sys.types WHERE is_user_defined = 1))
FROM sys.objects AS o
WHERE o.is_ms_shipped = 0
    AND o.type IN ('U', 'V', 'P', 'PC', 'FN', 'IF', 'TF', 'FS', 'FT', 'SO')
