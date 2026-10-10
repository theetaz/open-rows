# SQL Server showplan fixtures

Captured through the OpenRows TDS driver and core session actor from
`mcr.microsoft.com/mssql/server:2022-latest`, SQL Server build recorded in each
XML document. All objects and values are synthetic.

Re-capture: `OPENROWS_CAPTURE_SHOWPLAN=1 just test-mssql` (use the resource/cache
environment from the task). `sqlserver_explain_analyze_rollback_confirmation_and_session_restoration`
creates a 20,000-row heap `dbo.explain_t` and keyed `dbo.explain_keys`:

- `ms_scan.xml`: estimated whole-table scan.
- `ms_seek.xml`: estimated key lookup for id 10.
- `ms_join.xml`: actual hash join with `MAX_GRANT_PERCENT=0.001`, including
  native `SpillToTempDb` / `HashSpillDetails` warnings.
- `ms_update.xml`: actual UPDATE of ids 1–10, rolled back.

The runtime-counter/misestimate/spill unit supplement is explicitly synthetic;
it checks namespace prefixes, worker aggregation and warnings independently of
optimizer decisions in the captured plans.
