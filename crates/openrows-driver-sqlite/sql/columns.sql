SELECT name, type, "notnull", dflt_value, pk, hidden
FROM pragma_table_xinfo(?1, ?2) WHERE hidden <> 1 ORDER BY cid
