SELECT query_id, user, current_database, toString(address), query,
       toUInt64(elapsed * 1000), client_name
FROM system.processes ORDER BY elapsed DESC
