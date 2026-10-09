SELECT name, expr, type_full FROM system.data_skipping_indices
WHERE database = {p1:String} AND table = {p2:String} ORDER BY name
