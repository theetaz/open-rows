SELECT engine, total_rows, total_bytes, create_table_query, comment
FROM system.tables WHERE database = {p1:String} AND name = {p2:String}
