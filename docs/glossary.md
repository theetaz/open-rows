# Glossary

| Term | Definition |
| --- | --- |
| **ADBC** | Arrow Database Connectivity — a vendor-neutral API for database drivers that return Arrow data. Used post-1.0 for warehouses. |
| **ADR** | Architecture Decision Record — a short document capturing a significant decision, its context and consequences. |
| **Arrow / Arrow IPC** | Apache Arrow columnar in-memory format and its binary serialisation, used for result storage and transport. |
| **Capability** | A flag a driver declares (e.g. `server_cancel`, `editable_results`) that the UI uses to show or hide features. Also: Tauri 2's per-window permission sets. |
| **Change set** | Staged, undoable data edits (updates/inserts/deletes) against one table, applied atomically on commit. |
| **Conformance suite** | Shared tests every driver must pass for its tier. |
| **Driver** | Crate implementing the `Driver`/`Connection` traits for one database protocol. |
| **Driver host** | Separate helper process that runs FFI-based drivers so their crashes cannot take down the app. |
| **Edit target** | The table, key columns and column mapping that make a result set editable. |
| **Environment tag** | Label (local/dev/staging/prod/custom) with colour attached to a connection; drives safety defaults and visual warnings. |
| **Flavour** | A variant of a database family sharing a wire protocol (e.g. CockroachDB on PostgreSQL wire), with adapted introspection and dialect. |
| **Fingerprint** | Cheap value computed by a driver to detect schema changes without full re-introspection. |
| **Metadata session** | Dedicated connection used for introspection so it never queues behind user queries. |
| **MQL** | MongoDB Query Language as written in mongosh syntax; we parse a safe subset. |
| **Profile** | Saved connection definition (no secrets). |
| **Result store** | Core component holding result sets as Arrow batches with a memory budget and spill to disk. |
| **Safe mode** | Collective name for read-only, confirm-writes and dangerous-statement policies. |
| **Session** | One live database connection owned by one actor task. |
| **Session actor** | Tokio task that exclusively owns a connection and processes commands sequentially. |
| **Spill** | Writing cold result batches to temp files when memory budget is exceeded. |
| **Tile** | Block of 256 rows × visible columns transferred from core to grid. |
| **Tier** | Support level for a driver (1, 2, 3, Bridge) with defined test and feature commitments. |
| **Window (result)** | A row/column range of a result requested by the grid. |
| **Workspace** | A connection to a database server/host, containing multiple databases. Open databases keep independent tabs and schemas. |
| **Project** | A user folder with saved queries, API collections, shared connection definitions and settings, meant for git; metadata in `.openrows/project.yaml`, opened with Open Project Folder. |
