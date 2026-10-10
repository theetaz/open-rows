# 26 — Feature Inventory and Parity Plan

Research snapshot: **2026-10-06**, with **249 features across 12 areas**, **59 public sources**, **29 new P1 requirements** and **21 explicit gap decisions**. TablePlus is the primary benchmark. This is a requirements comparison, not a claim that OpenRows has shipped these features: the current repository contains the desktop shell, shared IDs and IPC bindings, while database workflows remain planned. Release acceptance still requires implementation and the gates in [19](19-roadmap.md) and [25](25-engineering-standards.md#3-acceptance-report).

## Reading the inventory

Each stable inventory key represents a distinct user capability or independently configurable behaviour. **Covered** means an existing functional requirement specifies it; **New** means a previously missing or partial 1.0 capability now has a requirement; **Partial** means a bounded alternative or narrower scope; **Gap Gxx** means intentionally deferred or excluded, with rationale below. References resolve to public source URLs in [Sources](#sources); the improvement column is an OpenRows proposal, not a measured advantage. M1–M6 are delivery milestones; all new rows are P1, due by 1.0 rather than expanding the first alpha's P0 exit gate.

The inventory covers the macOS benchmark unless qualified. TablePlus's [new overview][S01] warns that driver availability varies across platforms. Its older [docs overview][S02] omits newer drivers, and its older [AI page][S03] says rows are never shared; the [new AI page][S04] describes approved/read-only query results being sent to providers. Use the newer public documentation for these behaviours. Changelog version labels are recorded as published; do not infer release dates from their names. The task's maintainer-reported settings panes are all corroborated by [public settings documentation][S05]; no local application was inspected.

### Connections and secure routes

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| C01 | Profile forms | Save server details. | Covered FR-CONN-01 | M1 | Typed validation and diagnostics. | [S06] |
| C02 | Profile duplication | Clone a connection. | Covered FR-CONN-01 | M1 | Recheck environment and safety. | [S06] |
| C03 | Profile search | Find saved endpoints. | Covered FR-CONN-01 | M1 | Keyboard-first filtered list. | [S06] |
| C04 | Connection groups | Organise endpoints hierarchically. | Covered FR-CONN-01 | M1 | Text labels supplement icons. | [S06] |
| C05 | Connection colours | Distinguish server contexts. | Covered FR-CONN-06 | M1 | Colour plus environment text. | [S07] |
| C06 | Environment tags | Label deployment environments. | Covered FR-CONN-06 | M1 | Production defaults to confirmation. | [S07] |
| C07 | URL import | Populate a profile from a URI. | Covered FR-CONN-02 | M1 | Preview parsed fields, hide secrets. | [S07] |
| C08 | Connection test | Check connectivity before saving. | Covered FR-CONN-08; implemented for PostgreSQL (D-11: timed DNS, TCP/tunnel, SSH, TLS, auth, version steps with hints) | M1 | Diagnose each route stage. | [S08] |
| C09 | SSH password | Tunnel using password authentication. | Covered FR-CONN-04; implemented (D-10: password in keychain, keyboard-interactive MFA prompt, first-use host-key prompt) | M1 | MFA and host-key verification. | [S07] |
| C10 | SSH private key | Authenticate with an imported key. | Covered FR-CONN-04; implemented (D-10: key file + keychain passphrase, ssh-agent) | M1 | Agent support; passphrase in keychain. | [S07] |
| C11 | SSH host configuration | Use local SSH configuration. | Covered FR-CONN-04; implemented (D-10: `~/.ssh/config` alias as SSH host; `Match` not supported) | M1 | Explicit route and alias diagnostics. | [S07] |
| C12 | Bastion / jump routing | SSH hop enables private-server access; public pages do not establish multi-hop parity. | Covered FR-CONN-04, FR-NET-01; implemented (D-10: jump-host list or alias `ProxyJump`, per-hop test steps) | M1 | Visible multi-hop chain, per-hop errors. | [S07] (single-hop evidence only) |
| C13 | TLS modes | Select driver-specific verification policy. | Covered FR-CONN-05; implemented for PostgreSQL (D-11: disable … verify-full, unverified remote flagged in the sheet) | M1 | Verify identity by default. | [S06] |
| C14 | Client SSL key | Select authentication key material. | Covered FR-CONN-05; implemented for PostgreSQL (D-11: key file, passphrase in keychain) | M1 | Never export key contents. | [S06] |
| C15 | Client certificate | Supply a TLS client certificate. | Covered FR-CONN-05; implemented for PostgreSQL (D-11; expiry diagnostics pending) | M1 | Certificate details and expiry diagnostics. | [S06] |
| C16 | Custom CA | Trust a chosen certificate authority. | Covered FR-CONN-05, FR-NET-06; implemented for PostgreSQL (D-11; cloud CA bundles pending) | M1; cloud CAs M2 | Explain trust failures. | [S06] |
| C17 | Credential storage | Keep passwords in keychain. | Covered FR-CONN-03 | M1 | OS-native storage across desktop platforms. | [S07] |
| C18 | Credential prompts | Choose ephemeral password entry. | FR-CONN-13 implemented (M2-21: keychain, ask, none and command policies) | M2 | Separate storage policy from auth method. | [S06] |
| C19 | Credential command | Obtain short-lived credentials externally. | FR-CONN-13 implemented (M2-21: literal argv, local SHA-256 trust, timeout, cancel, scrubbed diagnostics) | M2 | Trust gate, timeout and scrubbed output. | [S06] |
| C20 | Startup SQL | Initialise the database session. | Covered FR-CONN-10 | M2 | Display bootstrap failures per session. | [S06] |
| C21 | Connection import | Restore profile files. | FR-CONN-09/12 implemented (M2-21: OpenRows JSON, pgpass, pg_service and my.cnf; conflict preview); Gap G01 for undocumented encrypted format | M2; G01 excluded | Public specifications only; conflict preview. | [S07] |
| C22 | Connection export | Transfer profiles or groups. | FR-CONN-12 implemented (M2-21: schema-validated v1 JSON, selected/group/all, no secrets) | M2 | Versioned open format without secrets. | [S07] |
| C23 | Password-bearing export | Include credentials in protected export. | Gap G02 | Excluded from 1.0 | Provision credentials independently in keychain. | [S06] |
| C24 | Keep-alive | Maintain idle connectivity. | New FR-APP-12; FR-NET-04 | M1 baseline; controls M2 | Sleep/wake probes avoid blind retries. | [S09] |

### Cloud and zero-trust routes (OpenRows extension, M2-24)

These FR-NET-03 rows document OpenRows scope; no TablePlus adoption/parity claim is inferred from the provider CLI documentation. Implementation details and public CLI references are in [docs/24 §15](24-networking-and-protocols.md#15-implementation-notes-m2-24-cloud-and-zero-trust-hops).

| Key | Feature | Status / FR | Target |
| --- | --- | --- | --- |
| NET01 | AWS SSM managed-node / remote-host forwarding | Implemented (M2-24): structured fields, Session Manager plugin discovery and managed process lifecycle; FR-NET-03 | M2 |
| NET02 | kubectl port-forward | Implemented (M2-24): context, namespace, resource, remote port and kubeconfig; FR-NET-03 | M2 |
| NET03 | Cloud SQL / AlloyDB auth proxy | Implemented (M2-24): instance, IAM auth, private/public IP, credential file path and encrypted-upstream TLS hint; FR-NET-03 | M2 |
| NET04 | cloudflared Access TCP | Implemented (M2-24): hostname and loopback forwarding; tokens remain in the tool configuration; FR-NET-03 | M2 |
| NET05 | Custom command hop | Implemented (M2-24b): absolute program, literal per-argument placeholders, local hop-purpose trust, timeout and shared lifecycle; untrusted exchange imports; FR-NET-03 | M2 |

### Workspace and database navigation

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| W01 | Concurrent databases | Keep several database contexts open. | New FR-WS-04 | M2 done (M2-20) | Context visible in every pane. | [S10] |
| W02 | Database switcher (⌘K) | Choose another database. | New FR-WS-04; FR-APP-01 | M2 done (M2-20) | Named palette action; see shortcut decision below. | [S11] |
| W03 | Connection switcher | Choose another server. | New FR-WS-04 | M2 done (M2-20) | Environment and safety preview. | [S10] |
| W04 | Open anything (⌘P) | Jump to objects or queries. | Covered FR-SCHEMA-02; new FR-WS-04 | M2 done (M2-20) | Cached search ≤ 50 ms. | [S12] |
| W05 | Workspaces per window | Separate contexts inside one window. | New FR-WS-04 | M2 done (M2-20) | Independent tabs and result lifetimes. | [S10] |
| W06 | Multiple windows | Open an independent working window. | Covered FR-APP-03 | M2 | Platform-native window behaviour. | [S10] |
| W07 | Move workspace/tab | Detach work into another window. | Deferred FR-APP-03, FR-APP-10 (M6-05/M5-11; no multi-window runtime) | M6 | Preserve context and pending changes. | [S10] |
| W08 | Preview object tab | Reuse an unmodified tab. | Implemented FR-APP-10 (M1-27, M2-22b dirty protection) | M2 | Dirty text cannot be replaced. | [S10] |
| W09 | Pin object tab | Keep an object open. | Implemented FR-APP-10 (M1-27 persistence) | M2 | Persistent pin plus accessible state. | [S10] |
| W10 | Rename tabs | Give editors meaningful labels. | Implemented FR-APP-10 (M1-27, M2-20 restore) | M2 | Labels survive layout restore. | [S10] |
| W11 | Close other/right tabs | Close groups of tabs. | Implemented FR-APP-10 (M2-22b single impact review) | M2 | One impact-aware dirty-state review. | [S10] |
| W12 | Restore session | Reopen previous workspace descriptors. | Covered FR-EDIT-01; new FR-WS-04 | M2 done (M2-20) | Lazy data loading; no silent SQL execution. | [S13] |
| W13 | Create database | Choose name, encoding and collation. | New FR-SCHEMA-12 | M2 | Capability-aware statement preview. | [S11] |
| W14 | Rename database | Rename when supported. | New FR-SCHEMA-12 | M2 | Explain unsupported engine operations. | [S11] |
| W15 | Drop database | Remove a database. | New FR-SCHEMA-12 | M2 | Explicit target and impact confirmation. | [S11] |

### Sidebar, saved queries and history

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| B01 | Object sections | Browse tables, views and routines. | Covered FR-SCHEMA-01, FR-SCHEMA-07 | M2 done (M2-09) | Lazy capability-driven sections and read-only inspectors, including safe MariaDB sequence details (docs/10 §7). | [S14] |
| B02 | Sidebar fuzzy search | Narrow the object list. | Covered FR-SCHEMA-02 | M1 | Worker search over cached metadata. | [S14] |
| B03 | Recent objects | Revisit recently opened items. | New FR-SCHEMA-09 | M2 done (M2-23) | Scoped persistent recents, 15 shown / 50 stored, dedupe and Clear (docs/10 §8). | [S14] |
| B04 | System schemas toggle | Hide or reveal internal schemas. | New FR-SCHEMA-09 | M2 done (M2-23) | Default-off per-profile system visibility, including SQLite internal names (docs/10 §8). | [S14] |
| B05 | Section visibility | Hide routine/recent sections. | New FR-SCHEMA-09 | M2 done (M2-23) | Persistent section menu; search and independent ⌘P remain available (docs/10 §8). | [S14] |
| B06 | Object favourites | Pin objects at the list's top. | New FR-SCHEMA-09 | M2 done (M2-23) | Persistent overload-aware identities, drag/keyboard order, missing rows with Remove (docs/10 §8). | [S14] |
| B07 | Context labels | Display database/connection names. | New FR-SCHEMA-09 | M2 done (M2-23 verification) | Environment never conveyed by colour alone. | [S14] |
| B08 | Query favourites | Save reusable named SQL. | Covered FR-EDIT-08 | M2 | Ordinary `.sql` files. | [S15] |
| B09 | Favourite folders | Organise saved SQL in folders. | New FR-EDIT-14 | M2 | Merge-friendly folder structure. | [S15] |
| B10 | Keyword expansion | Insert saved SQL via a keyword. | New FR-EDIT-14 | M2 | Insert without automatic execution. | [S15] |
| B11 | Favourite file actions | Rename, delete or reveal a query. | New FR-EDIT-14 | M2 | File-manager integration and dirty checks. | [S15] |
| B12 | Favourite autosave | Control writes to saved query files. | New FR-EDIT-14 | M2 | Separate from mandatory crash recovery. | [S15] |
| B13 | Query history | Record executed statements. | Covered FR-WS-01 | M1 | Duration/status/row count and search. | [S16] |
| B14 | History reuse | Insert, copy or open a past statement. | New FR-WS-05 | M2 done (M2-20) | Reapply current connection safety. | [S16] |
| B15 | History deletion | Delete entries or clear the list. | New FR-WS-05 | M2 done (M2-20) | Retention and sensitive-literal policy. | [S16] |
| B16 | History to favourite | Save past SQL for reuse. | New FR-WS-05; FR-EDIT-08 | M2 done (M2-20) | Review before persisting sensitive literals. | [S16] |
| B17 | Database value search | Locate matching data across tables. | Gap G03 | Post-1.0 | Explicit scope, load budget and cancellation. | [S17] |

### Grid, filters and cell inspection

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| G01 | Table pagination | Browse configurable row pages. | Covered FR-GRID-08 | M1 | Avoid compulsory exact counts. | [S18] |
| G02 | Header sort | Toggle sorting by a column. | Covered FR-GRID-04 | M1 | Distinguish server and local result sorting. | [S19] |
| G03 | Column selection | Hide/show chosen fields. | Covered FR-GRID-02 | M1 | Pin and reorder as well. | [S20] |
| G04 | Filter conditions | Apply several predicates. | Implemented FU-03; FR-GRID-11, FR-GRID-04 (docs/08 §11) | M2 | Typed predicates with AND/OR grouping. | [S20] |
| G05 | Filter operators | Use ranges, lists, NULL and patterns. | New FR-GRID-11 | M2 | Explain dialect and type applicability. | [S20] |
| G06 | Raw SQL filter | Enter a predicate directly. | New FR-GRID-11 | M2 | Classification and ordinary safety checks. | [S20] |
| G07 | Any-column filter | Match a value across fields. | New FR-GRID-11 | M2 | Explicit potentially expensive operation. | [S20] |
| G08 | Cell quick filter | Filter from a selected value. | New FR-GRID-11 | M2 | Parameterised generated predicates. | [S20] |
| G09 | Header quick filter | Start with a chosen column. | New FR-GRID-11 | M2 | Keyboard equivalent for context menu. | [S20] |
| G10 | Filter SQL preview | Inspect the generated SELECT. | New FR-GRID-11 | M2 | Copyable dialect-correct query. | [S20] |
| G11 | Filter reset | Return to unfiltered data. | New FR-GRID-11 | M2 | Show active scope before export. | [S20] |
| G12 | Remember table state | Restore filters, layout and sort. | Implemented FU-03: version-two grouped filters and migration; FR-GRID-12 | M2 | Versioned state, separate from secrets. | [S20], [S05] |
| G13 | Row detail form | Inspect a wide row vertically. | Covered FR-GRID-09 | M2 | Accessible field-by-field navigation. | [S21] |
| G14 | Inspector field search | Find a field in a wide row. | New FR-GRID-14 | M2 | Preserve active-cell focus. | [S21] |
| G15 | Bulk field edit | Change selected rows together. | New FR-GRID-14 | M2 | Affected-count and change-set review. | [S21] |
| G16 | Quick look | Inspect large JSON/blob values. | Covered FR-GRID-06 | M1 | JSON tree, hex and image previews. | [S22] |
| G17 | Copy cell/column | Copy values at different scopes. | Covered FR-GRID-03; new FR-GRID-13 | M1; controls M2 | Scope explicit in commands. | [S19] |
| G18 | Plain/CSV copy | Put rows on the clipboard. | Covered FR-GRID-03 | M1 | TSV and configurable escaping. | [S19] |
| G19 | CSV headers | Include or omit column names. | New FR-GRID-13 | M2 | Explicit header policy. | [S19] |
| G20 | JSON copy | Copy selected rows as JSON. | Covered FR-GRID-03 | M1 | Preserve numbers and NULL values. | [S19] |
| G21 | Markdown copy | Copy a readable table. | Covered FR-GRID-03 | M1 | Escape delimiters and multiline values. | [S19] |
| G22 | HTML copy | Copy a formatted HTML table. | New FR-GRID-13 | M2 | Escape markup; no executable content. | [S19] |
| G23 | SQL INSERT copy | Produce row insertion statements. | Covered FR-GRID-03 | M1 | Correct dialect literals and quoting. | [S19] |
| G24 | Foreign-key jump | Open the referenced row. | Implemented FU-03: outgoing cross-schema/composite keys; same-schema incoming only; FR-GRID-07 | M2 | Composite-key support and visible destination. | [S20] |
| G25 | Background result loading | Browse before fetching finishes. | Covered FR-GRID-01 | M1 | Arrow windows, backpressure and disk spill. | [S23] |

### Data writes and safety

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| D01 | Inline data edits | Change a cell in place. | Covered FR-DATA-01 | M1 | Require identifiable single-table rows. | [S19] |
| D02 | Insert rows | Stage newly entered rows. | Covered FR-DATA-01 | M1 | Type-aware validation. | [S19] |
| D03 | Duplicate rows | Create copies of selected rows. | Covered FR-DATA-01 | M1 | Generated keys handled by driver. | [S19] |
| D04 | Delete rows | Stage row removal. | Covered FR-DATA-01 | M1 | Visible pending deletion state. | [S19] |
| D05 | Editable query results | Change values in result grids. | Covered FR-DATA-01 | M1 | Read-only explanation when key mapping is unsafe. | [S24] |
| D06 | Commit changes | Apply staged GUI edits. | Covered FR-DATA-02, FR-DATA-03 | M1 | Transactional rollback where supported. | [S25] |
| D07 | Discard changes | Remove pending GUI edits. | New FR-DATA-08 | M2 | Confirm impact; distinguish executed SQL. | [S26] |
| D08 | Review generated SQL | Inspect writes before committing. | Covered FR-DATA-03 | M1 | Same preview path across drivers. | [S27] |
| D09 | Unrestricted mode | Send queries without prompts. | FR-APP-11 implemented (M2-22a) | M2 implemented (M2-22a) | Production cannot inherit this silently. | [S28] |
| D10 | Confirm every query | Warn on each execution. | FR-APP-11 implemented (M2-22a) | M2 implemented (M2-22a) | Classification and target shown. | [S28] |
| D11 | Confirm writes | Warn only on mutating operations. | FR-CONN-07 implemented (M2-22a) | M1; M2-22a explicit policy | Read-only mode also available. | [S28] |
| D12 | Authenticate every query | Require authentication for execution. | FR-APP-11 implemented (M2-22a) | M2 implemented (M2-22a) | OS-native authentication when available. | [S28] |
| D13 | Authenticate writes | Authenticate non-read operations. | FR-APP-11 implemented (M2-22a) | M2 implemented (M2-22a) | Parsed classification, not keyword-only rules. | [S28] |
| D14 | Biometric safe unlock | Use OS authentication for prompts. | FR-APP-11 implemented (M2-22a) | M2 implemented (M2-22a) | Explicit fallback on unsupported hardware. | [S28] |
| D15 | Startup app lock | Require unlock on launch. | FR-APP-11 implemented (M2-22a) | M2 implemented (M2-22a) | Lock applies to all windows and switches. | [S28] |
| D16 | Inactivity app lock | Lock after time in background. | FR-APP-11 implemented (M2-22a) | M2 implemented (M2-22a) | Context switches cannot bypass it. | [S28] |

### Structure and database objects

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| T01 | Structure inspection | Inspect column/index metadata. | Covered FR-SCHEMA-03 | M1 | Cached metadata with refresh controls. | [S18] |
| T02 | Object DDL | View table creation SQL. | Covered FR-SCHEMA-04 | M1 | No plugin required. | [S18] |
| T03 | Table creation | Design a new table. | New FR-SCHEMA-10 | M2 | Preview engine-specific DDL. | [S18] |
| T04 | Table rename | Change a table's name. | New FR-SCHEMA-10 | M2 | Invalidate affected metadata. | [S18] |
| T05 | Table truncate | Remove all table rows. | New FR-SCHEMA-10 | M2 | Show transaction limitations and impact. | [S18] |
| T06 | Table drop | Remove a table definition. | New FR-SCHEMA-10 | M2 | Explicit destructive confirmation. | [S18] |
| T07 | Column creation | Add typed columns. | Covered FR-SCHEMA-05 | M2 | Review defaults and nullability. | [S29] |
| T08 | Column alteration | Change column attributes. | Covered FR-SCHEMA-05 | M2 | Warn about conversions and locking. | [S29] |
| T09 | Column removal | Drop selected columns. | Covered FR-SCHEMA-05 | M2 | Identify dependent constraints. | [S29] |
| T10 | Index creation | Add an index. | Covered FR-SCHEMA-05 | M2 | Capability-aware options. | [S30] |
| T11 | Index alteration | Change an existing index. | Covered FR-SCHEMA-05 | M2 | Explain rebuild when no ALTER exists. | [S30] |
| T12 | Index removal | Drop an index. | Covered FR-SCHEMA-05 | M2 | Surface constraint dependencies. | [S30] |
| T13 | Nullability constraints | Set required/optional fields. | Covered FR-SCHEMA-05 | M2 | Use database semantics, not UI-label inference. | [S31] |
| T14 | Primary keys | Define row identity. | Covered FR-SCHEMA-05 | M2 | Composite-key support. | [S31] |
| T15 | Foreign keys | Define inter-table relationships. | Covered FR-SCHEMA-05 | M2 | Review actions and target columns. | [S31] |
| T16 | Column defaults | Define default expressions. | Covered FR-SCHEMA-05 | M2 | Distinguish expressions from literals. | [S31] |
| T17 | Trigger creation | Add trigger definitions. | New FR-SCHEMA-11 | M2 | Dialect validation and DDL review. | [S32] |
| T18 | Trigger edit/drop | Maintain existing triggers. | New FR-SCHEMA-11 | M2 | Same safety policy as other DDL. | [S32] |
| T19 | Views and routines | Edit stored object definitions. | Partial FR-SCHEMA-07; new FR-SCHEMA-11 | M2 viewing done (M2-09) | Read-only inspectors, stored definitions and Open in Editor; lifecycle editing remains separately gated. | [S33] |
| T20 | Relationship diagram | Visualise table relationships. | Covered FR-SCHEMA-06 | M2 | Original layout and open SVG export. | [S34] |
| T21 | Diagram structure editing | Modify tables and keys visually. | Partial FR-SCHEMA-06; Gap G21 | M2 read-only; editing post-1.0 | Reuse reviewed change sets when added. | [S34] |

### Query editor and results

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| E01 | SQL highlighting | Colour dialect syntax. | Covered FR-EDIT-02 | M1 | Dialect modules load on demand. | [S35] |
| E02 | Schema completion | Suggest names and SQL keywords. | Covered FR-EDIT-02 | M1 | Alias-aware, cached completion ≤ 50 ms. | [S36] |
| E03 | Completion preferences | Set case, qualification and acceptance key. | New FR-EDIT-13 | M2 | Preserve highlighting if suggestions are off. | [S36] |
| E04 | Run current | Execute the cursor's statement. | Covered FR-EDIT-03 | M1 | Dialect-aware statement detection. | [S37] |
| E05 | Run selection | Execute selected SQL. | Covered FR-EDIT-03 | M1 | Clearly show execution scope. | [S37] |
| E06 | Run all | Execute an editor script. | Covered FR-EDIT-03 | M1 | Per-statement results and status. | [S37] |
| E07 | Cancel query | Stop active execution. | Covered FR-EDIT-04 | M1 | Send cancellation within 200 ms. | [S37] |
| E08 | Beautify SQL | Apply readable formatting. | Covered FR-EDIT-05 | M2 | Configurable style, worker execution. | [S38] |
| E09 | Minify SQL | Compact formatted statements. | Gap G04 | Post-1.0 | Parse-aware transformation if added. | [S38] |
| E10 | Split editor panes | Work in independent editors. | New FR-EDIT-11 | M2 | Restore pane context and keyboard focus. | [S39] |
| E11 | Separate result tabs | Keep each statement's result separate. | Covered FR-EDIT-03 | M1 | Virtualise inactive result views. | [S40] |
| E12 | Stacked results | Display results in one view. | New FR-EDIT-15 | M2 | Lazy sections avoid large DOMs. | [S41] |
| E13 | Parameter prompts | Enter values for placeholders. | Covered FR-EDIT-06 | M2 | Bind typed values instead of text substitution. | [S42] |
| E14 | Alternative placeholder formats | Accept additional template syntaxes. | Partial FR-EDIT-06; Gap G05 | M2 standard forms; others post-1.0 | Avoid accidental substitution inside strings. | [S42] |
| E25 | Explain / Explain Analyze | Inspect estimates, actuals and hotspots. | Implemented FR-EDIT-07 (M2-06, FU-02) | M2 | Lazy tree/Flame/Raw, exclusive measures, accessible navigation; SQL Server showplan alongside PG/MySQL/MariaDB/SQLite; confirmed write rollback and read-only refusal. | [S41] |
| E15 | Multiple carets | Edit several positions at once. | New FR-EDIT-12 | M2 | Retain ≤ 16 ms typing budget. | [S43] |
| E16 | Occurrence/block selection | Select repeated or rectangular text. | New FR-EDIT-12 | M2 | Predictable undo grouping. | [S43] |
| E17 | Comment shortcuts | Toggle line or block comments. | New FR-EDIT-12 | M2 | Dialect-aware tokens. | [S44] |
| E18 | Folding | Collapse code regions. | New FR-EDIT-12 | M2 | Keep navigation and screen-reader context. | [S45] |
| E19 | Wrap and whitespace | Control long lines and invisibles. | New FR-EDIT-12 | M2 | Independent, accessible display settings. | [S41] |
| E20 | Vim mode | Use modal editor keybindings. | Covered FR-EDIT-09 | M2 | Commands coexist with app focus navigation. | [S41] |
| E21 | Stop on script error | Choose whether failures halt a script. | New FR-EDIT-15 | M2 | Stop by default; show skipped statements. | [S41] |
| E22 | Current-statement highlight | Show which statement will execute. | New FR-EDIT-15 | M2 | Match the splitter used for execution. | [S46] |
| E23 | Server notices | Show database notices. | New FR-EDIT-15 | M2 | Scrubbed non-blocking notification stream. | [S46] |
| E24 | Save SQL file | Export editor text. | Covered FR-EDIT-08 | M2 | Portable UTF-8 Project files. | [S37] |

### Import, export, backup and restore

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| I01 | CSV import | Load delimited data into tables. | Covered FR-IO-02 | M2 | Bulk-load paths and bounded batches. | [S47] |
| I02 | JSON import | Load JSON records. | Covered FR-IO-02 | M2 | Also support streamed NDJSON. | [S47] |
| I03 | Mapping preview | Choose destination columns. | Covered FR-IO-02; new FR-IO-06 | M2 | Type errors and skipped columns explicit. | [S47] |
| I04 | Create table on import | Infer a new table or use text columns. | New FR-IO-06 | M2 | Review inference before DDL. | [S47] |
| I05 | Import conflicts | Choose supported duplicate-key policy. | New FR-IO-06 | M2 | Capability-gated policy, clear partial-failure report. | [S47] |
| I06 | CSV conventions | Configure encoding, quoting and delimiters. | New FR-GRID-13, FR-IO-05, FR-IO-06 | M2 | Reject ambiguous decimal/delimiter combinations. | [S48] |
| I07 | CSV export | Save table/result rows. | Covered FR-IO-01 | M1 | Stream at ≥ 80% of DB send rate. | [S24] |
| I08 | JSON export | Save rows as JSON. | Covered FR-IO-01 | M1 | NDJSON for incremental consumption. | [S24] |
| I09 | SQL export | Save rows/definitions as SQL. | Covered FR-IO-01; new FR-IO-05 | M1 data; M2 scope/options | Review structure/data/drop scope. | [S47] |
| I10 | Excel export | Write spreadsheet workbooks. | Covered FR-IO-01 | M2 | Explicit precision/type policy. | [S47] |
| I11 | Column-filtered export | Export only chosen fields. | New FR-IO-05 | M2 | Redaction and scope preview. | [S47] |
| I12 | Multi-object export | Export several tables/views. | New FR-IO-05 | M2 | Stream each object; no UI materialisation. | [S47] |
| I13 | Current-page export | Export the displayed page. | New FR-IO-05 | M2 | Distinguish page from whole dataset. | [S08] |
| I14 | SQL file execution | Run SQL loaded from disk. | Covered FR-IO-03 | M2 | Stop-on-error, progress and cancellation. | [S49] |
| I15 | Database backup | Write a database dump. | Covered FR-IO-04; built in M2-14 + FU-03 scope picker/tree actions (docs/05 §15) | M2 | Detect installed tool version and compatibility. | [S50] |
| I16 | Database restore | Restore from a dump. | Covered FR-IO-04; built in M2-14 + FU-03 scope picker/tree actions (docs/05 §15) | M2 | Target confirmation and typed tool errors. | [S50] |
| I17 | Background transfers | Continue working during file jobs. | New FR-IO-05 | M2 | Backpressure, visible progress and cancel. | [S47] |

### Administration, metrics and plugins

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| A01 | Process list | Inspect server activity. | Covered FR-ADMIN-01 | M2 | Separate admin connection and capability labels. | [S51] |
| A02 | Cancel/kill session | Stop selected server work. | Covered FR-ADMIN-01 | M2 | Explain cancel versus session termination. | [S51] |
| A03 | Activity sort/refresh | Reorder and reload process rows. | Covered FR-ADMIN-01, FR-GRID-04 | M2 | Preserve selection and sort. | [S51] |
| A04 | User/grant viewer | Inspect accounts and permissions. | Covered FR-ADMIN-02 | M2 done (M2-09) | PG/MySQL/MariaDB/SQL Server attributes, direct memberships and explicit grants; no computed effective permissions (docs/10 §7). | [S52] |
| A05 | User lifecycle | Create, change or remove users. | Partial FR-ADMIN-02; Gap G06 | Post-1.0 editor | Review privilege changes as SQL. | [S52] |
| A06 | Privilege editor | Edit global/database/table grants. | Partial FR-ADMIN-02; Gap G06 | Post-1.0 editor | Least-privilege impact summary. | [S52] |
| A07 | Account resource limits | Set per-account usage limits. | Gap G06 | Post-1.0 | Driver-specific controls, no false portability. | [S52] |
| A08 | SQL bar/line charts | Build charts from SQL results. | Gap G07; FR-ADMIN-03 covers server metrics only | Post-1.0 | Bounded read-only workloads and open board files. | [S53] |
| A09 | Dashboard tables | Add a query-backed table widget. | Gap G07 | Post-1.0 | Reuse streamed result pipeline. | [S53] |
| A10 | Dashboard inputs | Parameterise widgets with input fields. | Gap G07 | Post-1.0 | Typed bindings, no string injection. | [S53] |
| A11 | Dashboard refresh | Refresh on timer, event or request. | Gap G07 | Post-1.0 | Load limits and visible polling state. | [S53] |
| A12 | Dashboard transfer | Import/export metric boards. | Gap G07 | Post-1.0 | Documented, merge-friendly formats. | [S45] |
| A13 | Plugin manager | Install additional functionality. | Gap G08 | Post-1.0 | WASM permissions and resource limits. | [S54] |
| A14 | Script extension API | Write custom plugins. | Gap G08 | Post-1.0 | Capability-limited portable components. | [S54] |
| A15 | Formatter extension | Add an external SQL formatter. | Partial FR-EDIT-05 built-in; Gap G08 for plugins | M2 built-in; plugins post-1.0 | Formatting available without extensions. | [S54] |
| A16 | DDL/migration extension | Generate table scripts/framework migrations. | Partial FR-SCHEMA-04 DDL; Gap G09 migrations | M1 DDL; migrations post-1.0 | Original exporters with licence notices. | [S54] |
| A17 | Cell URL extension | Open cell URLs or email links. | Gap G10 | Post-1.0 | User gesture, scheme validation and confirmation. | [S54] |

### Settings, appearance, shortcuts and platform integration

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| P01 | General pane | Configure startup and update behaviour. | Implemented FR-APP-08, FR-APP-12 (M2-22b; feeds require M1-31) | M2 | Searchable settings and validated defaults. | [S05] |
| P02 | Table pane | Configure table presentation. | Implemented FR-APP-08 (M2-22b); FR-GRID-12 reused M2-19 | M2 | Explicit estimated/exact count controls. | [S05] |
| P03 | Editor pane | Configure completion and editing. | Implemented FR-APP-08 (M2-22b); editor controls reused | M2 | One source for per-editor defaults. | [S05] |
| P04 | Fonts & Themes pane | Adjust visual appearance. | Implemented FR-APP-02, FR-APP-13 (M2-22b) | M1 defaults; M2 customisation | Accessible original token themes. | [S55] |
| P05 | Security pane | Configure locks and safety defaults. | Implemented FR-APP-08, FR-APP-11 (M2-22a reused) | M2 | Production policy remains visible. | [S05] |
| P06 | Locations pane | Choose local data directories. | Implemented FR-APP-08 (M2-22b) | M2 | Validate moves; never silently sync history. | [S05] |
| P07 | Keymap pane | Rebind keyboard actions. | Implemented FR-APP-01 (existing rebinding reused) | M1 | Conflict diagnostics and reset defaults. | [S56] |
| P08 | LLM Agent pane | Configure assistant providers. | Covered FR-AI-01; new FR-APP-08 | M4.5 | Policy, provider health and context preview. | [S05] |
| P09 | MCP pane | Configure external client access. | Covered FR-AI-06; new FR-AI-10 | M4.5 | Scoped read-only access. | [S57] |
| P10 | Light/dark/system | Follow a chosen or OS appearance. | Covered FR-APP-02 | M1 | High contrast also supported. | [S58] |
| P11 | Custom theme JSON | Duplicate, import and export themes. | Implemented FR-APP-13 (M2-22b) | M2 | Documented open tokens, no theme code. | [S55] |
| P12 | Git theme repositories | Install/update remote theme collections. | Gap G11 | Post-1.0 | Licence and trust review before install. | [S55] |
| P13 | Editor font/line height | Adjust editor text metrics. | Implemented FR-APP-02, FR-APP-13 (M2-22b) | M2 | Preserve typing performance and scaling. | [S55] |
| P14 | Grid/sidebar fonts | Adjust table and sidebar density. | Implemented FR-APP-02, FR-APP-13 (M2-22b) | M2 | Named density modes and accessible targets. | [S55] |
| P15 | Syntax/state colours | Customise code and edit-state colours. | Implemented FR-APP-13 (M2-22b) | M2 | Contrast validation plus non-colour indicators. | [S55] |
| P16 | Row striping/scrollbars | Adjust grid visual aids. | New FR-GRID-12 | M2 | Respect OS scrollbar preferences. | [S59] |
| P17 | Estimate counts | Avoid expensive exact counts. | New FR-GRID-12 | M2 | Label estimates, exact count only on request. | [S59] |
| P18 | Query timeouts | Set a maximum execution duration. | Implemented FR-APP-12 (M2-22b new-run timeout) | M2 | All long operations cancellable. | [S09] |
| P19 | Beta update choice | Opt into a prerelease channel. | Implemented FR-APP-12 preferences/endpoint selection (M2-22b); signed distribution M1-31 | M2 | Signed channel manifests and rollback guidance. | [S13] |
| P20 | Update opt-out | Disable automatic updates/notices. | Implemented FR-APP-12 notice preference (M2-22b); updater backend M1-31 | M1 baseline; M2 controls | Free, explicit user controls. | [S13] |
| P21 | Crash-report control | Configure crash reporting. | Implemented FR-APP-12 preference (M2-22b); no reporting backend | M2 | Off by default, scrubbed and never paywalled. | [S28] |
| P22 | Navigation shortcuts | Move among tabs, panes and workspaces. | Covered FR-APP-01; new FR-APP-10 | M1 actions; M2 contexts | Rebindable platform defaults. | [S44] |
| P23 | Menu shortcuts | Trigger file, editor and grid actions. | Covered FR-APP-01; new FR-APP-14 | M1/M2 | Keyboard operability with visible focus. | [S44] |
| P24 | CLI open | Launch a connection through the shell. | New FR-APP-09 | M5 | OpenRows command with secret-free profiles. | [S07] |
| P25 | Object/filter deep links | Open a table at a chosen scope. | New FR-APP-09 | M5 | Preview target; never auto-run arbitrary SQL. | [S07] |
| P26 | File associations | Open local database files. | Implemented FR-APP-14 handoffs/associations (M2-22b); packaged OS verification pending | M2 macOS; M6 others | Validate content before opening. | [S08] |
| P27 | Native macOS app | Integrate with Mac window/menu conventions. | New FR-APP-14; NFR-PORT-01 | M1 shell; M2 integration | ≤ 800 ms cold-start budget. | [S35] |
| P28 | Apple Silicon support | Run natively on ARM Macs. | New FR-APP-07; NFR-PORT-01 | M1 | Same conformance on Intel and ARM. | [S35] |
| P29 | Windows/Linux apps | Offer desktop builds for other OSes. | New FR-APP-07, FR-APP-14; NFR-PORT-01 | M6 | Publish driver capabilities per platform. | [S01] |
| P30 | iOS client | Manage databases on mobile. | Gap G12 | Outside 1.0 desktop scope | Portable core leaves future options open. | [S01] |
| P31 | Toolbar customisation | Select visible toolbar actions. | Gap G13 | Post-1.0 | Same commands accessible through palette. | [S52] |

### AI, LLM integration and MCP

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| L01 | Assistant panel | Chat beside database content. | Covered FR-AI-04 | M4.5 | Off until configured; visible tool trace. | [S04] |
| L02 | API-key providers | Choose a hosted model vendor. | Covered FR-AI-01 | M4.5 | Keys in keychain; provider allowlists. | [S04] |
| L03 | Local models | Connect to Ollama/local servers. | Covered FR-AI-02 | M4.5 | No dummy API-key requirement. | [S04] |
| L04 | Compatible endpoints | Configure another model API. | Covered FR-AI-01 | M4.5 | Proxy/CA support shared with DB routes. | [S04] |
| L05 | Installed CLI bridge | Use an installed coding CLI as provider. | Covered FR-AI-03 | M4.5 | Versioned adapters and bounded lifecycle. | [S04] |
| L06 | Custom ACP provider | Attach a protocol-compatible local assistant. | Partial FR-AI-03; Gap G14 | M4.5 named bridges; ACP post-1.0 | Add protocol adapter only after trust tests. | [S04] |
| L07 | Copilot sign-in | Use a subscription via account login. | Gap G15 | Post-1.0 | User-owned provider adapters, no token extraction. | [S04] |
| L08 | Model/provider selection | Change the conversation backend. | Covered FR-AI-01, FR-AI-04 | M4.5 | Show model and sharing policy together. | [S04] |
| L09 | Schema context selection | Attach chosen table definitions. | Covered FR-AI-08 | M4.5 | Exact payload preview and relevance ranking. | [S04] |
| L10 | Generate/explain queries | Ask for SQL or explanations. | Covered FR-AI-04 | M4.5 | Dialect-aware proposals, no write execution. | [S04] |
| L11 | Assistant query execution | Let the assistant obtain query results. | Partial FR-AI-06, FR-AI-09; Gap G16 for writes | M4.5 reads; writes excluded | Bounded read tools; writes open as proposals. | [S04] |
| L12 | Result sharing | Send executed query results to a model. | Covered FR-AI-08 | M4.5 | Off/schema/samples/results policy and redaction. | [S04] |
| L13 | MCP server enable/status | Turn on local client access. | Covered FR-AI-06; new FR-AI-10 | M4.5 | Loopback transport, disabled by default. | [S57] |
| L14 | MCP client setup | Show supported transport configuration. | Covered FR-AI-06; new FR-AI-10 | M4.5 | Open protocol, secret-free setup guidance. | [S57] |
| L15 | Scoped tokens | Limit tokens to allowed connections. | New FR-AI-10 | M4.5 | Read-only scopes, no write token. | [S57] |
| L16 | Token revocation | Disable access credentials. | New FR-AI-10 | M4.5 | Cancel active requests and disconnect clients. | [S57] |
| L17 | Integration activity | Inspect requests and connected clients. | Covered FR-AI-08; new FR-AI-10 | M4.5 | Scrubbed metadata and retention controls. | [S57] |
| L18 | Remote TLS MCP | Serve clients on other machines. | Gap G17 | Excluded from 1.0 | Localhost endpoint limits exposure. | [S57] |
| L19 | MCP write permissions | Allow external clients to mutate data. | Gap G16 | Excluded | No write tool, including unrestricted connections. | [S57] |

### Supported database families

Each row is engine access, not a promise of identical editing/admin capabilities. [Driver tiers](05-database-drivers.md#3-support-tiers) govern OpenRows support; **FR-APP-07** supplies the explicit functional mapping. The newer public overview lists all families below, with macOS as its reference. Oracle is macOS-only there; Snowflake is documented for macOS/Windows; DynamoDB is marked beta.

| Key | Feature | What it does | OpenRows status / FR | Target | OpenRows improvement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| DB01 | PostgreSQL | Access PostgreSQL databases. | New FR-APP-07 | M1 | Binary decoding and conformance. | [S01] |
| DB02 | MySQL | Access MySQL databases. | New FR-APP-07; dogfood slice in D-12 (browse, query, edit; SSH/TLS) | M1 | Streaming and typed values. | [S01] |
| DB03 | MariaDB | Access MariaDB databases. | New FR-APP-07; dogfood slice in D-12 (own engine entry, detected from the version) | M1 | Detected flavour semantics. | [S01] |
| DB04 | SQLite | Open local SQLite databases. | New FR-APP-07; dogfood slice in D-12 (local files, read-only open, create new file) | M1 | Interruptible dedicated-thread execution. | [S01] |
| DB05 | SQL Server | Access TDS servers. | Implemented FR-APP-07 (M2-02), FR-EDIT-07 (FU-02) | M2 | TDS cancellation/TLS plus estimated/actual showplan, confirmed rolled-back writes and shared tree/Flame/Raw views (docs/05 §14). | [S01] |
| DB06 | ClickHouse | Access analytical datasets. | Implemented FR-APP-07 (M2-03; docs/05 §17) | M2 | HTTP(S) native Arrow streaming, routed TLS, query parameters, catalogs and cancellation; read-only grids, no transactional imports, dump/restore or parsed explain. Tested 25.3.9.72 LTS. | [S01] |
| DB07 | DuckDB | Query embedded analytical databases. | New FR-APP-07; host protocol/supervisor infrastructure implemented (M2-01a), SQLite process fixtures only; DuckDB, core adapter and packaging pending | M2 | Isolated native driver host. | [S01] |
| DB08 | libSQL | Access libSQL/Turso databases. | FR-APP-07 implemented (M2-05): own Hrana 3 HTTP remote client and SQLite-compatible local file flavour; bound values, guarded edits and catalogs | M2 | Explicit local/remote capabilities; bounded/truncated results; remote Explain/dump Unsupported, cross-origin sticky endpoints require reconnect (docs/05 §18). | [S01] |
| DB09 | MongoDB | Browse document collections. | New FR-APP-07; FR-DATA-07 | M3 | EJSON validation, no arbitrary JS. | [S01] |
| DB10 | Redis | Access typed key/value data. | New FR-APP-07; FR-DATA-07 | M3 | SCAN, bounded buffers and type editors. | [S01] |
| DB11 | Cassandra | Access wide-column data. | New FR-APP-07 | M5 | Paging and complete-key edits. | [S01] |
| DB12 | Oracle | Access Oracle databases. | New FR-APP-07 | M5 beta | User-installed client, isolated host. | [S01] |
| DB13 | Redshift | Access warehouse databases. | New FR-APP-07 | M5 beta | Explicit protocol-subset capabilities. | [S01] |
| DB14 | CockroachDB | Access PG-compatible databases. | Partial FR-APP-07 | M1 protocol; M2 flavour quirks | Detect flavour, test deviations. | [S01] |
| DB15 | Snowflake | Access cloud warehouse data. | Gap G18 | Post-1.0 bridge | ADBC and explicit driver licensing. | [S01] |
| DB16 | BigQuery | Access hosted analytical data. | Gap G18 | Post-1.0 bridge | Visible query cost and bounded reads. | [S01] |
| DB17 | DynamoDB | Access AWS-managed items. | New FR-APP-07 | M5 beta | Capacity reporting and AWS profiles. | [S01] |
| DB18 | Elasticsearch | Access search indexes. | New FR-APP-07 | M5 beta | Native DSL and OpenSearch flavour. | [S01] |
| DB19 | Cloudflare D1 | Access hosted SQLite-family data. | Gap G19 | Post-1.0 | Treat HTTP/auth differences explicitly. | [S01] |
| DB20 | Greenplum | Access distributed PG-family data. | Gap G20 | Post-1.0 validation | Dedicated catalog/conformance profile. | [S01] |
| DB21 | Vertica | Access columnar warehouse data. | Gap G20 | Post-1.0 validation | Do not infer support from wire similarity. | [S01] |
| DB22 | SingleStore | Access MySQL-family data. | Gap G20 | Post-1.0 validation | Dedicated flavour tests. | [S01] |
| DB23 | Compatible databases | Connect through a shared wire protocol. | Partial FR-APP-07; Gap G20 for untested flavours | M1 baseline; certified flavours M2/post-1.0 | Publish tested versions, not blanket compatibility. | [S02] |

## Where we go further

These are design commitments, not comparative performance measurements:

- **Performance:** [13](13-performance.md) and NFR-PERF-01–10 define cold start ≤ 800 ms, idle RSS ≤ 200 MB, cached object search ≤ 50 ms, typing ≤ 16 ms, no grid frame > 32 ms and streamed CSV ≥ 80% of server send rate. Arrow windows, bounded channels and disk spill keep results out of React state. Verify on reference hardware; no TablePlus timing was measured here.
- **Safety:** FR-DATA-03/04 add transactional application and optimistic conflicts; FR-CONN-05 verifies TLS identity by default. FR-AI-09 and [ADR-0009](adr/0009-native-ai-byok-cli-bridges-mcp.md) exclude model/MCP write tools. TablePlus documents approval-based writes and a silent-mode exception for MCP; OpenRows keeps writes as editor proposals ([public MCP policy][S57]).
- **AI and privacy:** FR-AI-01–10 combine local/BYOK providers, context preview, per-connection sharing levels, provider allowlists, redaction and scoped read access. MCP client support is separately specified by FR-AI-07; this research does not establish an equivalent TablePlus MCP client.
- **UX and motion:** [20](20-ux-foundations.md) specifies context labels, keyboard paths, visible focus, density modes and functional motion ≤ 150 ms, disabled under reduced motion. Preview tabs, splits and filter controls must retain those rules rather than copying proprietary layouts.
- **Open formats and breadth:** FR-WS-03, FR-EDIT-08, FR-CONN-12 and FR-APP-13 keep queries, profiles and themes inspectable; FR-IO-01 adds NDJSON/Parquet beyond the core CSV/JSON/SQL workflow. TablePlus also publicly documents SQL files and JSON themes; the goal is documented interoperability throughout OpenRows. FR-API-01–11 add the integrated API client. All features remain free under [22](22-governance-and-licensing.md).

## Gaps and scope decisions

### 1.0 gaps now specified

These requirements are new in this task. They close specification gaps, not implementation gaps; milestone acceptance must test the full text in [02](02-requirements.md), not just the short inventory descriptions.

| Requirement IDs | Gap closed | Placement |
| --- | --- | --- |
| FR-CONN-12–13 | Open connection transfer and credential policy/commands | M2 |
| FR-SCHEMA-09–12 | Sidebar controls, table/database lifecycle and definition editing | M2 |
| FR-EDIT-11–15 | Splits, editing primitives, completion controls, favourite actions, execution/display settings | M2 |
| FR-GRID-11–14 | Compound/raw filters, persistent presentation, copy options, bulk inspector | M2 |
| FR-DATA-08 | Explicit staged-edit discard | M2 |
| FR-IO-05–06 | Transfer scopes, background jobs and table-creating import | M2 |
| FR-WS-04–05 | Database contexts/window layout and history actions | M2 |
| FR-AI-10 | Scoped MCP tokens, revocation and client activity | M4.5 |
| FR-APP-07–14 | Driver scope, settings, deep links, tabs, locks, update controls, themes and OS integration | M1 baseline; M2 controls; M5 links; M6 platforms |

### Deferred, excluded and bounded alternatives

| Gap | Inventory keys | Decision and rationale | Roadmap |
| --- | --- | --- | --- |
| G01 | C21 | Do not reverse-engineer `.tableplusconnection` encryption/serialization. Import only a publicly specified format; use OpenRows exports for its own round-trip. | Excluded unless public specification appears |
| G02 | C23 | Secret-bearing profile exports conflict with keychain-only portability. 1.0 exports omit secrets; encrypted credential transfer needs a separate security decision. | Excluded from 1.0 |
| G03 | B17 | Cross-table value search needs workload limits and a search plan; fuzzy object-name search is not data search. | Post-1.0 |
| G04 | E09 | SQL minification is optional; formatting is required. | Post-1.0 |
| G05 | E14 | Standard named/positional bound parameters are required; extra templating syntaxes and remembered values are optional. | Post-1.0 |
| G06 | A05–A07 | Keep the existing FR-ADMIN-02 boundary: viewer in 1.0, user/grant/resource-limit editing later. Raw SQL remains available under safety checks. | Post-1.0 |
| G07 | A08–A12 | User dashboards differ from FR-ADMIN-03 server metrics. Both are deferred; dashboard charts/inputs/refresh/transfer require their own scope. | Post-1.0 |
| G08 | A13–A15 | Plugin manager/runtime follows [21](21-extensibility.md); built-in formatting remains M2. | Post-1.0 |
| G09 | A16 | DDL viewing is required; framework migration exporters are optional extensions. | Post-1.0 |
| G10 | A17 | Opening arbitrary cell links requires URI trust policy. | Post-1.0 |
| G11 | P12 | Local JSON themes are required; remote Git theme installation and updates need a trust/licensing design. | Post-1.0 |
| G12 | P30 | iOS is outside the defined desktop distribution scope. | Future candidate, no commitment |
| G13 | P31 | Toolbar rearrangement is optional; all commands stay reachable by palette/menu. | Post-1.0 |
| G14 | L06 | Named CLI bridges do not imply generic ACP support. | Post-1.0 |
| G15 | L07 | Subscription account integration needs a provider-specific auth/terms decision. | Post-1.0 |
| G16 | L11, L19 | Approval-based AI/MCP writes intentionally differ from the accepted read/propose-only design. | Excluded by ADR-0009 |
| G17 | L18 | Remote TLS MCP listener intentionally differs from the loopback-only 1.0 design. | Excluded from 1.0 |
| G18 | DB15–DB16 | Snowflake/BigQuery follow the existing ADBC warehouse bridge plan. | Post-1.0 |
| G19 | DB19 | D1 is not interchangeable with SQLite or libSQL; add a dedicated authenticated HTTP driver later. | Post-1.0 |
| G20 | DB20–DB23 | Greenplum, Vertica, SingleStore and other untested flavours need catalog and protocol conformance work before claiming support. | Post-1.0; existing tested flavours keep docs/05 tiers |
| G21 | T21 | The existing 1.0 ER diagram is read-only; visual table/key editing is an existing post-1.0 candidate. | Post-1.0 |

Shortcut decision: OpenRows [20](20-ux-foundations.md#2-information-architecture) uses ⌘K for the database switcher, ⌘P for quick-open and ⌘⇧P for the command palette; FR-APP-01 makes bindings configurable. Projects in FR-WS-03 are portable folders of saved queries and collections; FR-WS-04 adds window contexts (Workspace = connection) without conflating those concepts.

## Sources

All URLs below were accessed **2026-10-06**. Public text only: no binaries, private APIs, screenshots, themes or other assets downloaded for the inventory. Behaviour is paraphrased; no prose quotations are needed. Legacy docs are retained for the behaviours they document; newer pages take precedence for changed behaviour. The changelog supports E18 (folding, 26.9.10) and A12 (board transfer, 26.6.0). No benchmark or cross-platform equality is inferred from marketing claims.

| Ref | Public page URL |
| --- | --- |
| S01 | <https://tableplus.com/docs/overview> |
| S02 | <https://docs.tableplus.com/> |
| S03 | <https://docs.tableplus.com/llm-plugin> |
| S04 | <https://tableplus.com/docs/llm-plugin> |
| S05 | <https://tableplus.com/docs/preferences/general> |
| S06 | <https://tableplus.com/docs/gui-tools/manage-connections> |
| S07 | <https://docs.tableplus.com/gui-tools/manage-connections> |
| S08 | <https://tableplus.com/docs/getting-started> |
| S09 | <https://docs.tableplus.com/preferences/general/connections> |
| S10 | <https://docs.tableplus.com/gui-tools/the-interface/multi-tabs-workspaces-windows> |
| S11 | <https://docs.tableplus.com/gui-tools/untitled> |
| S12 | <https://docs.tableplus.com/gui-tools/open-anything> |
| S13 | <https://docs.tableplus.com/preferences/general/application> |
| S14 | <https://docs.tableplus.com/gui-tools/the-interface/left-sidebar> |
| S15 | <https://docs.tableplus.com/query-editor/keyword-binding-favorite> |
| S16 | <https://docs.tableplus.com/query-editor/query-history> |
| S17 | <https://tableplus.com/docs/gui-tools/search-in-database> |
| S18 | <https://docs.tableplus.com/gui-tools/working-with-table/table> |
| S19 | <https://docs.tableplus.com/gui-tools/working-with-table/row> |
| S20 | <https://tableplus.com/docs/gui-tools/filter> |
| S21 | <https://docs.tableplus.com/gui-tools/the-interface/right-sidebar> |
| S22 | <https://docs.tableplus.com/gui-tools/the-interface/quick-look> |
| S23 | <https://docs.tableplus.com/query-editor/streaming-results-and-async-loading> |
| S24 | <https://docs.tableplus.com/query-editor/working-with-query-results> |
| S25 | <https://docs.tableplus.com/gui-tools/code-review-and-safemode/commit-changes> |
| S26 | <https://tableplus.com/docs/gui-tools/code-review-and-safemode/discard-changes> |
| S27 | <https://docs.tableplus.com/gui-tools/code-review-and-safemode/code-preview> |
| S28 | <https://tableplus.com/docs/preferences/crash-and-security> |
| S29 | <https://docs.tableplus.com/gui-tools/working-with-table/column> |
| S30 | <https://docs.tableplus.com/gui-tools/working-with-table/index> |
| S31 | <https://docs.tableplus.com/gui-tools/working-with-table/constraint> |
| S32 | <https://docs.tableplus.com/gui-tools/working-with-table/trigger> |
| S33 | <https://tableplus.com/docs/gui-tools/database-objects> |
| S34 | <https://tableplus.com/docs/gui-tools/diagram> |
| S35 | <https://tableplus.com/> |
| S36 | <https://tableplus.com/docs/query-editor/autocomplete> |
| S37 | <https://docs.tableplus.com/query-editor/untitled> |
| S38 | <https://docs.tableplus.com/query-editor/query-reformat> |
| S39 | <https://docs.tableplus.com/query-editor/split-panes> |
| S40 | <https://docs.tableplus.com/query-editor/split-results-into-tabs> |
| S41 | <https://tableplus.com/docs/query-editor/customize-query-editor> |
| S42 | <https://tableplus.com/docs/query-editor/query-parameters> |
| S43 | <https://tableplus.com/docs/query-editor/multiple-carets> |
| S44 | <https://docs.tableplus.com/utilities/shortcut-keys> |
| S45 | <https://tableplus.com/blog/2017/02/changelogs.html> |
| S46 | <https://docs.tableplus.com/query-editor/customize-query-editor> |
| S47 | <https://tableplus.com/docs/gui-tools/import-and-export> |
| S48 | <https://tableplus.com/docs/preferences/general/csv-file> |
| S49 | <https://docs.tableplus.com/gui-tools/import-and-export> |
| S50 | <https://docs.tableplus.com/gui-tools/backup-and-restore> |
| S51 | <https://tableplus.com/docs/gui-tools/process-list> |
| S52 | <https://tableplus.com/docs/gui-tools/user-management> |
| S53 | <https://docs.tableplus.com/gui-tools/metrics-board> |
| S54 | <https://docs.tableplus.com/utilities/plugin> |
| S55 | <https://tableplus.com/docs/preferences/fonts-and-colors> |
| S56 | <https://docs.tableplus.com/preferences/keymap> |
| S57 | <https://tableplus.com/docs/preferences/mcp> |
| S58 | <https://docs.tableplus.com/preferences/fonts-and-colors> |
| S59 | <https://docs.tableplus.com/preferences/general/table-data> |

[S01]: https://tableplus.com/docs/overview
[S02]: https://docs.tableplus.com/
[S03]: https://docs.tableplus.com/llm-plugin
[S04]: https://tableplus.com/docs/llm-plugin
[S05]: https://tableplus.com/docs/preferences/general
[S06]: https://tableplus.com/docs/gui-tools/manage-connections
[S07]: https://docs.tableplus.com/gui-tools/manage-connections
[S08]: https://tableplus.com/docs/getting-started
[S09]: https://docs.tableplus.com/preferences/general/connections
[S10]: https://docs.tableplus.com/gui-tools/the-interface/multi-tabs-workspaces-windows
[S11]: https://docs.tableplus.com/gui-tools/untitled
[S12]: https://docs.tableplus.com/gui-tools/open-anything
[S13]: https://docs.tableplus.com/preferences/general/application
[S14]: https://docs.tableplus.com/gui-tools/the-interface/left-sidebar
[S15]: https://docs.tableplus.com/query-editor/keyword-binding-favorite
[S16]: https://docs.tableplus.com/query-editor/query-history
[S17]: https://tableplus.com/docs/gui-tools/search-in-database
[S18]: https://docs.tableplus.com/gui-tools/working-with-table/table
[S19]: https://docs.tableplus.com/gui-tools/working-with-table/row
[S20]: https://tableplus.com/docs/gui-tools/filter
[S21]: https://docs.tableplus.com/gui-tools/the-interface/right-sidebar
[S22]: https://docs.tableplus.com/gui-tools/the-interface/quick-look
[S23]: https://docs.tableplus.com/query-editor/streaming-results-and-async-loading
[S24]: https://docs.tableplus.com/query-editor/working-with-query-results
[S25]: https://docs.tableplus.com/gui-tools/code-review-and-safemode/commit-changes
[S26]: https://tableplus.com/docs/gui-tools/code-review-and-safemode/discard-changes
[S27]: https://docs.tableplus.com/gui-tools/code-review-and-safemode/code-preview
[S28]: https://tableplus.com/docs/preferences/crash-and-security
[S29]: https://docs.tableplus.com/gui-tools/working-with-table/column
[S30]: https://docs.tableplus.com/gui-tools/working-with-table/index
[S31]: https://docs.tableplus.com/gui-tools/working-with-table/constraint
[S32]: https://docs.tableplus.com/gui-tools/working-with-table/trigger
[S33]: https://tableplus.com/docs/gui-tools/database-objects
[S34]: https://tableplus.com/docs/gui-tools/diagram
[S35]: https://tableplus.com/
[S36]: https://tableplus.com/docs/query-editor/autocomplete
[S37]: https://docs.tableplus.com/query-editor/untitled
[S38]: https://docs.tableplus.com/query-editor/query-reformat
[S39]: https://docs.tableplus.com/query-editor/split-panes
[S40]: https://docs.tableplus.com/query-editor/split-results-into-tabs
[S41]: https://tableplus.com/docs/query-editor/customize-query-editor
[S42]: https://tableplus.com/docs/query-editor/query-parameters
[S43]: https://tableplus.com/docs/query-editor/multiple-carets
[S44]: https://docs.tableplus.com/utilities/shortcut-keys
[S45]: https://tableplus.com/blog/2017/02/changelogs.html
[S46]: https://docs.tableplus.com/query-editor/customize-query-editor
[S47]: https://tableplus.com/docs/gui-tools/import-and-export
[S48]: https://tableplus.com/docs/preferences/general/csv-file
[S49]: https://docs.tableplus.com/gui-tools/import-and-export
[S50]: https://docs.tableplus.com/gui-tools/backup-and-restore
[S51]: https://tableplus.com/docs/gui-tools/process-list
[S52]: https://tableplus.com/docs/gui-tools/user-management
[S53]: https://docs.tableplus.com/gui-tools/metrics-board
[S54]: https://docs.tableplus.com/utilities/plugin
[S55]: https://tableplus.com/docs/preferences/fonts-and-colors
[S56]: https://docs.tableplus.com/preferences/keymap
[S57]: https://tableplus.com/docs/preferences/mcp
[S58]: https://docs.tableplus.com/preferences/fonts-and-colors
[S59]: https://docs.tableplus.com/preferences/general/table-data

### Implemented (M2-20): W01–W05, W12 and B14–B16

Existing independent connection/database contexts and ⌘K database switching are retained. Connection switching previews environment, safety and routing; ⌘P adds cached local/project queries to objects (50 ms p95 gate at 10k + 1k). Per-pane context labels include connection/database, including both split panes. Versioned descriptor restoration and Reopen Previous Session are disconnected and never execute SQL; inspector/table identities and split layouts restore lazily without results or credentials. History reuse routes through current connection safety, adds cursor insertion and an editable literal review before project/local saving, confirmed scoped/all clearing, count/age/forever retention and production/read-only/explicit lexer redaction. See docs/09, docs/12, docs/15 §11 and docs/20. W06–W07 remain independent window work; W08–W11 reuse prior tab operations and are not expanded by this task.

### Implemented (M2-22a): D09–D16

Six explicit policies, lossless legacy decoding, production-safe imports/copies,
parser-based confirmation/authentication, native macOS device-owner authentication,
and shared startup/inactivity lock enforcement are implemented. ConfirmAll shows
statement classification and connection/database. Unsupported platforms visibly
fall back to the matching confirmation policy and disable lock/auth controls.
All data IPC entry points share the Rust lock guard; mounted content and portals
are hidden in every locked window. Settings groups/locations, tabs/layout, general
controls, themes and file associations are completed by M2-22b below. Native macOS
Touch ID/system-password UX remains the maintainer's manual acceptance check.

## Implemented (M2-22b): application parity

W08–W11 and P01–P07/P11/P13–P15/P18–P20/P26 reuse the baseline and add strict
portable settings, highlighted search, confirmed resets, restart-only copy/rollback
locations with explicit history consent, one bulk dirty-tab review, independent
font roles and versioned safe theme tokens, general preferences/new-run timeout,
and content-validated native handoffs without SQL execution. See docs/07, docs/15,
docs/16 and docs/20. Existing P16/P17 grid presentation/count controls, keyboard
navigation, P23 native menu shortcuts and export reveal actions are retained.
P08/P09 remain M4.5; P12 remains G11. W07 needs multi-window M6-05/M5-11.
P27 startup/native distribution and P29 Windows/Linux validation retain their
separate milestones. Null signed-feed configuration requires M1-31 release
engineering before any actual updates.
