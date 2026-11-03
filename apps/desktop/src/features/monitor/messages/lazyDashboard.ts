// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { t } = defineMessages({
  keepMonitorAlert:
    'Keep this monitor and its alert rules running after the tab closes, while OpenRows is open. Locking or pausing stops it.',
  background: 'Background',
  sessionPid: 'Session {pid}',
  pidPid: 'PID {pid}',
  stateWaitQuery:
    '{state} · {wait} · query age {age_ms} ms · transaction age {transaction_ms} ms',
  unknownBlockerOutside: 'Unknown blocker (outside visibility)',
  cycle: 'Cycle',
  depthLimit: 'Depth limit',
  blockerBlockedQuery:
    'Blocker → blocked. Query and transaction ages are shown; lock-wait duration is unavailable. Unseen owners, cycles and capped chains remain explicit.',
  blockingChains: 'Blocking chains',
  noBlockingEdges: 'No blocking edges in this sample.',
  graphDisplayLimit: 'Graph display limit (100 nodes)',
  localMonitorHistory: 'Local monitor history',
  keepLocalMonitor: 'Keep local monitor history',
  localHistoryEnabled: 'Local history enabled.',
  historyDisabledPurged: 'History disabled and purged.',
  offDefaultRedacted:
    'Off by default. Redacted scalar aggregates only; no SQL text or result rows. Retention 24 hours, 50 MiB per connection and 100 MiB app-wide. Oldest observations expire first. Collection stops when Monitor closes. The local store is not encrypted; database names may be sensitive.',
  connectionHistoryPurged: 'Connection history purged.',
  purgeConnectionHistory: 'Purge connection history',
  allMonitorHistory: 'All monitor history purged.',
  purgeAllMonitor: 'Purge all monitor history',
  incidents: 'Incidents',
  rulesEvaluateLive:
    'Rules evaluate live samples while this monitor runs. Unknown or missing samples never open or resolve an incident. Notifications stay in OpenRows.',
  noIncidentsApp: 'No incidents in this app session.',
  incidentList: 'Incident list',
  status: 'Status',
  rule: 'Rule',
  scope: 'Scope',
  started: 'Started',
  ended: 'Ended',
  evidence: 'Evidence',
  open: 'Open',
  resolved: 'Resolved',
  titleCritical: '{title} · critical',
  server: 'Server',
  database: 'Database',
  rulesCount: 'Rules ({count})',
  holdFireafterRecover: 'Hold {fireAfter}; recover {clearAfter}',
  titleTrendMissing: '{title} trend; missing observations are gaps',
  titleValueValue2:
    '{title} — {value} to {value2}; unknown/reset observations are gaps',
  dataTableTitle: 'Data table for {title}',
  titleObservations: '{title} observations',
  collected: 'Collected',
  value: 'Value',
  state: 'State',
  unknown: 'Unknown',
  previousRows: 'Previous rows',
  valueValue2Count: '{value}–{value2} of {count} · Partial source',
  valueValue2Count2: '{value}–{value2} of {count}',
  nextRows: 'Next rows',
  liveInterval: 'Live interval',
  monitorInterval: 'Monitor interval',
  off: 'Off',
  valueS: '{value} s',
  timeWindow: 'Time window',
  monitorTimeWindow: 'Monitor time window',
  live10Minutes: 'Live · 10 minutes',
  historyValueH: 'History · {value} h',
  resumeMonitor: 'Resume monitor',
  pauseMonitor: 'Pause monitor',
  refreshHistoryRange: 'Refresh history range',
  databaseStatementsI:
    'Database/statements/I/O ≥15 s; tables/indexes/replication ≥60 s',
  exportReport: 'Export report…',
  pausedRetainedSamples: 'Paused — retained samples are stale',
  live: 'Live',
  stateAwaitingFirst: '{state} · Awaiting first sample',
  stateAgeS: '{state} · {age} s since last sample',
  rowsReadReturned:
    'Rows read = returned + fetched; rows written = inserted + updated + deleted. Cache ratio uses interval block hits/reads.',
  serverStatusIncludes:
    'Server status includes all databases and collector queries. Rows are InnoDB counters; buffer pool hit ratio is 1 − physical reads/read requests. Missing engine counters remain unknown.',
  requestCountersCover:
    'Request counters cover statements run from OpenRows editors and grids against this file. File gauges come from cheap PRAGMAs, file sizes and the WAL index; integrity and quick checks never run automatically. Missing WAL values mean the file is not in WAL mode.',
  ownOpenrowsSessions:
    'Own OpenRows sessions only — other applications are not visible.',
  mongodbMonitoringNot: 'MongoDB monitoring is not available yet.',
  performanceCountersCover:
    'Performance counters cover the whole instance, including collector queries. CPU is the default resource pool share; buffer cache hit ratio and page life expectancy are current values. Missing counters remain unknown.',
  nodeCountersSystem:
    'Node counters from system events, metrics and asynchronous metrics include collector queries. Events are interval rates; running queries, merges, parts and memory are gauges. Missing or denied counters remain unknown.',
  requestCountersRound:
    'Request counters and round trip are measured by OpenRows. Server CPU, memory, storage and other clients’ queries are unavailable over Hrana and are never estimated.',
  ownOpenrowsRequests:
    'Own OpenRows requests only — server-wide metrics unavailable over Hrana.',
  updatedAfterQuery:
    'Updated after query completion. Request counters cover statements run from OpenRows editors and grids. Storage, memory, temp and thread values are read on the owning session after the running statement; a busy session leaves a gap.',
  ownOpenrowsSession:
    'Own OpenRows session only — other processes are not visible.',
  storedAggregateHistory:
    'Stored aggregate history; each bucket preserves a missing/reset state. Missing collection intervals are gaps. Other panels show live source tables. {note}',
  liveTenMinute:
    'Live ten-minute window. Database counters need two samples; no baseline or a reset means unknown. {note}',
  scopeScopeState: '{scope} scope · {state} · {value}',
  scopeScopeState2: '{scope} scope · {state}',
  previousWindowPrevious: 'Previous window: {previous}',
  filterSessions: 'Filter sessions',
  filterSessions2: 'Filter sessions…',
  countSessionsQuery:
    '{count} sessions. Query literals are redacted. Showing the first 100; filter to narrow the list.',
  countSessionsQueryLiterals: '{count} sessions. Query literals are redacted.',
  monitoringSettings: 'Monitoring settings',
  overview: 'Overview',
  sessions: 'Sessions',
  blocking: 'Blocking',
  topQueries: 'Top queries',
  advisor: 'Advisor',
  tables: 'Tables',
  indexes: 'Indexes',
  replication: 'Replication',
  iO: 'I/O',
  askAssistant: 'Ask assistant',
  boards: 'Boards',
  tableIO: 'Table I/O',
  indexIO: 'Index I/O',
  fileIO: 'File I/O',
  tableUsage: 'Table usage',
  indexUsage: 'Index usage',
  waits: 'Waits',
  monitoringAiAssistant:
    'Monitoring and the AI assistant are not available for MongoDB connections. They work with SQL databases.',
  monitorDataHidden: 'Monitor data hidden while the app is locked.',
  disconnectedConnectStart:
    'Disconnected. Connect to start monitoring; collection has stopped.',
  monitorDatabase: 'Monitor on {database}',
  monitorPanel: 'Monitor panel',
  localHistorySettings: 'Local history settings',
  ownQueries: 'Own queries',
  activePartsPer:
    'Active parts per table in the selected database. Many parts per partition slow merges and inserts; sizes are current, not rates.',
  statisticsSinceLast:
    'Statistics since last reset. Scan/tuple counters are cumulative; rate columns cover the latest valid interval. Vacuum/analyze times are unknown when the server reports NULL.',
  seekScanLookup:
    'Seek/scan/lookup/update counts since instance start or database open; rates cover the latest interval. Indexes unused since then are absent, which never proves an index can be dropped.',
  instrumentedTableIndex:
    'Instrumented table/index I/O since reset; rates cover the latest interval. Unused/redundant sys evidence is optional. Zero observed reads never proves an index can be dropped. Vacuum/bloat and per-query memory are unavailable.',
  noReplicationSenders:
    'No replication senders or receivers on this server; replication lag is unavailable.',
  unknownAwaitingBaseline: 'Unknown — awaiting a baseline or server value',
  gapNoObservation: 'Gap — no observation',
  statisticsResetAwaiting: 'Statistics reset — awaiting a new baseline',
  permissionDenied: 'Permission denied',
  unavailable: 'Unavailable',
  noSuccessfulSample: 'No successful sample',
  mathSSince: '{seconds} s since last successful sample',
  partialCoverage: 'Partial coverage',
  sampleDurationmsMs: 'Sample {durationMs} ms',
  copied: 'Copied',
  copySetupGuidance: 'Copy setup guidance',
  reviewGuidanceDba:
    'Review this guidance with your DBA. OpenRows never executes it automatically.',
  noWait: 'No wait',
  stateWithReason: '{state}: {reason}',
  scopeLabel: '{scope} scope',
  everySeconds: 'Every {cadence} s',
});
