-- User sessions in ProcessInfo::from_text_row order: id, login, database,
-- client address, program, status, wait, start, duration (ms), statement
-- text and whether it is this session; the longest-running first.
SELECT s.session_id, s.login_name,
    DB_NAME(COALESCE(r.database_id, s.database_id)), c.client_net_address,
    s.program_name, COALESCE(r.status, s.status), r.wait_type,
    CONVERT(nvarchar(30), COALESCE(r.start_time, s.last_request_start_time), 121),
    -- start_time and SYSDATETIME() differ in precision: clamp at zero.
    CASE WHEN r.start_time IS NULL THEN NULL
        WHEN r.start_time >= SYSDATETIME() THEN 0
        ELSE DATEDIFF_BIG(millisecond, r.start_time, SYSDATETIME()) END,
    t.text, CASE WHEN s.session_id = @@SPID THEN 1 ELSE 0 END
FROM sys.dm_exec_sessions AS s
LEFT JOIN sys.dm_exec_requests AS r ON r.session_id = s.session_id
LEFT JOIN sys.dm_exec_connections AS c
    ON c.session_id = s.session_id AND c.net_transport <> N'Session'
OUTER APPLY sys.dm_exec_sql_text(COALESCE(r.sql_handle, c.most_recent_sql_handle)) AS t
WHERE s.is_user_process = 1
ORDER BY COALESCE(r.start_time, s.last_request_start_time), s.session_id
