SELECT database, name,
       multiIf(engine = 'View', 'view', engine = 'MaterializedView', 'materialized_view', 'table'),
       total_rows, comment
FROM system.tables WHERE database = {p1:String}
UNION ALL
SELECT {p1:String}, name, 'function', NULL, '' FROM system.functions WHERE origin = 'SQLUserDefined'
ORDER BY name LIMIT 50000
