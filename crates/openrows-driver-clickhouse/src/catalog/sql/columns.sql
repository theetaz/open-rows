SELECT name, type, default_kind, default_expression, comment, is_in_primary_key
FROM system.columns WHERE database = {p1:String} AND table = {p2:String} ORDER BY position
