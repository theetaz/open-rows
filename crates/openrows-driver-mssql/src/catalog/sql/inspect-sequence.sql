-- SPDX-License-Identifier: Apache-2.0
-- Catalog values only: never NEXT VALUE FOR or sp_sequence_get_range.
SELECT TYPE_NAME(q.user_type_id), CONVERT(nvarchar(max), q.start_value),
    CONVERT(nvarchar(max), q.increment), CONVERT(nvarchar(max), q.minimum_value),
    CONVERT(nvarchar(max), q.maximum_value), q.is_cycling,
    CONVERT(nvarchar(max), q.cache_size), CONVERT(nvarchar(max), q.last_used_value), q.is_cached
FROM sys.sequences q JOIN sys.schemas s ON s.schema_id = q.schema_id
WHERE s.name = @P1 AND q.name = @P2
