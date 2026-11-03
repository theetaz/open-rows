// SPDX-License-Identifier: Apache-2.0
import { defineMessages } from '@openrows/ui/i18n';

export const { rich, t } = defineMessages({
  evidenceChangedEditor:
    'Evidence changed or the editor queue is unavailable; refresh the advisor.',
  optimisationAdvisor: 'Optimisation advisor',
  proposalsOpenEditor:
    'Proposals open an editor for review. Evidence age and coverage limit advice; plan costs do not measure runtime.',
  productionReviewLocks:
    'Production: review locks, I/O, storage and write cost before running any proposal.',
  noSuggestionsSufficient:
    'No suggestions with sufficient current evidence. Review Explain for scan/join evidence.',
  restoreDismissedSuggestions: 'Restore dismissed suggestions',
  advisorSuggestions: 'Advisor suggestions',
  strongConfidenceConfidence: '{strong} · {confidence} confidence',
  evidenceTradeOffs: 'Evidence and trade-offs',
  collectedValue: 'Collected {value}',
  viewEvidence: 'View evidence',
  openedEditor: 'Opened in editor',
  openSqlEditor: 'Open SQL in editor',
  hypopgWhatIf: 'HypoPG what-if',
  dismiss: 'Dismiss',
  snooze1Hour: 'Snooze 1 hour',
  hypopgAbsentDenied:
    'HypoPG is absent, denied, or the evidence changed. Review grants and refresh.',
  optionalOffDefault:
    'Optional and off by default. Requires HypoPG already installed in this database. A separate session compares estimated plan costs, resets hypothetical indexes, then closes. No extension or real index is created.',
  comparePlans: 'Compare plans',
  close: 'Close',
  reviewedWhatIf: 'Reviewed what-if SQL',
  whatIfSchema: 'What-if schema',
  statementBindingsReviewed: 'Statement and bindings reviewed',
  whatIfSql: 'What-if SQL reviewed',
  enableHypopgCheck: 'Enable HypoPG for this check',
  enableHypopg: 'Enable HypoPG',
  comparingEstimatedPlans: 'Comparing estimated plans…',
  confirmWhatIf: 'Confirm what-if plan',
  safetyPolicyRequires:
    'Your safety policy requires confirmation for this reviewed plan.',
  confirmPlanRequest: 'Confirm plan request',
  estimatedTotalCost:
    'Estimated total cost: {baselineCost} before → {hypotheticalCost} with hypothetical index. This is not a measured performance improvement.',
  retainedInvestigationAccounting: 'Retained investigation accounting',
  newest1000:
    'Newest 1,000 records. Purge through Settings → AI → Data sharing → Audit.',
  auditUnavailable: 'Audit unavailable.',
  loadAccounting: 'Load accounting',
  scrubbedUsageOutcomes: 'Scrubbed usage and outcomes',
  providerModel: 'Provider/model',
  outcome: 'Outcome',
  tokenCeiling: 'Token ceiling',
  costMillionths: 'Cost millionths',
  evidenceIds: 'Evidence ids',
  askAboutHealth:
    'Ask about health, slow queries, blocking locks or regressions. Findings cite the evidence ids they came from. Plain model text is an unverified interpretation.',
  openAssistant: 'Open assistant',
  scheduledReadOnly: 'Scheduled read-only investigations (off by default)',
  scheduledProvider: 'Scheduled provider',
  chooseProvider: 'Choose provider',
  scheduledModel: 'Scheduled model',
  modelId: 'Model id',
  runsUseOnly:
    'Runs use only the read and propose tools selected above and never write. Cost is a conservative estimate from your maximum input/output price, in millionths of the provider currency; unknown pricing cannot enable a schedule. Locking the app or turning AI off pauses every schedule. Results stay in app memory; scrubbed accounting is retained locally.',
  enableScheduledInvestigation: 'Enable scheduled investigation',
  resultsUnavailable: 'Results unavailable.',
  refreshScheduledResults: 'Refresh scheduled results',
  intervalSeconds300: 'Interval seconds (300–86400)',
  maximumRuns1: 'Maximum runs (1–1000)',
  steps112: 'Steps (1–12)',
  tokens165536: 'Tokens (1–65536)',
  deadlineSeconds1: 'Deadline seconds (1–60)',
  maximumCostMillionths: 'Maximum cost millionths per run',
  maximumPriceMillionths: 'Maximum price millionths per million tokens',
  investigationLimits: 'Investigation limits',
  permittedReadPropose: 'Permitted read/propose tools',
  queryEvidenceUnavailable: 'Query evidence unavailable; start monitoring.',
  queryInvestigation: 'Query investigation',
  queryInvestigationStatementid: 'Query investigation · {statementId}',
  liveWindowCalls:
    'Live window: {calls} calls · {totalMs} ms total · weighted mean {meanMs} ms · max since reset {maxSinceResetMs} ms.',
  p95P99Unavailable:
    'p95/p99 unavailable: pg_stat_statements supplies summaries, not observed distributions. Per-query memory and CPU attribution are unavailable. Trends cover the retained live window; persisted history contains database aggregates only. Partial coverage: baselines, gaps or evictions are present.',
  p95P99UnavailablePg:
    'p95/p99 unavailable: pg_stat_statements supplies summaries, not observed distributions. Per-query memory and CPU attribution are unavailable. Trends cover the retained live window; persisted history contains database aggregates only. Complete retained interval coverage.',
  queryMeanLatency: 'Query mean latency (ms)',
  regressionAdjacentComplete:
    'Regression: adjacent complete intervals each have ≥ 5 calls, mean ≥ 2× previous and increase ≥ 10 ms.',
  regressionValuePreviousmeanms:
    'Regression at {value}: {previousMeanMs} → {currentMeanMs} ms mean.',
  queryIntervalDeltas: 'Query interval deltas',
  collected: 'Collected',
  state: 'State',
  calls: 'Calls',
  totalMs: 'Total ms',
  rowsReturnedAffected: 'Rows returned/affected',
  bufferHits: 'Buffer hits',
  bufferReads: 'Buffer reads',
  tempWritesBlocks: 'Temp writes (blocks)',
  tempReadsBlocks: 'Temp reads (blocks)',
  unknown: 'unknown',
  relatedWaitsBlockers: 'Related waits and blockers',
  approximateCurrentSample:
    'Approximate current-sample association by normalized SQL; identities and cross-source snapshots may differ.',
  noMatchingSession: 'No matching session in the visible activity sample.',
  sessionPid: 'Session {pid}',
  waitWaitQuery: '· wait {wait} · query age {age_ms} ms · blockers {blockers}',
  reviewSqlBefore: 'Review SQL before Explain',
  collectedLiteralsBindings:
    'Collected literals and bindings are redacted. Supply a complete statement. Diagnostics use a separate session on {database}, with search path pg_catalog and the reviewed schema; profile startup SQL and your open transaction are excluded.',
  reviewedSql: 'Reviewed SQL',
  diagnosticSchema: 'Diagnostic schema',
  statementReviewed: 'Statement reviewed',
  confirmDiagnosticExecution: 'Confirm diagnostic execution',
  explainAnalyseExecutes:
    'Explain analyse executes this reviewed statement. Rollback cannot undo every effect, including sequences and external functions.',
  safetyPolicyRequiresConfirmation:
    'Your safety policy requires confirmation before requesting this plan.',
  productionWarningMeasured:
    'Production warning: measured execution can add load, take locks and affect production data. Review workload impact before confirming.',
  cancelDiagnostic: 'Cancel diagnostic',
  confirmMeasuredExecution: 'Confirm measured execution',
  requestingDiagnostic: 'Requesting diagnostic…',
  explainPlanOnly: 'Explain plan only',
  explainAnalyseRuns: 'Explain analyse (runs SQL)',
  scheduledInvestigation: 'Scheduled investigation',
  providerModelJob: '{provider} / {model} · {job} · {runs}/{maxRuns} runs',
  outcomeTokensTokens:
    '{outcome} · ≤ {tokens} tokens · ≤ {costMicros} cost millionths',
  waitingNextInterval: 'Waiting for the next interval',
  running: 'Running',
  finished: 'Finished',
  stopped: 'Stopped',
  pausedWhileApp: 'Paused while the app is locked',
  pausedWhileAi: 'Paused while AI is off',
  paused: 'Paused',
  runFailedStart: 'Run failed to start.',
  runNow: 'Run now',
  resumeFailed: 'Resume failed.',
  pauseFailed: 'Pause failed.',
  resumeSchedule: 'Resume schedule',
  pauseSchedule: 'Pause schedule',
  cancellationFailed: 'Cancellation failed.',
  removeSchedule: 'Remove schedule',
  cancelSchedule: 'Cancel schedule',
  unverifiedInterpretationDelta: 'Unverified interpretation: {delta}',
  callsTotalTime:
    'Calls, total time, rows and I/O/scan counters are rates for the latest valid interval. Window mean is total/calls. Maximum and cumulative mean are statistics since last reset. Rankings use summed covered live-window deltas; unknown/reset intervals remain unknown.',
  topQueries: 'Top queries',
  copied: 'Copied',
  copyNormalisedSql: 'Copy normalised SQL',
  openEditorReview: 'Open in editor for review',
  selectStatementSafely:
    'Select a statement with safely normalised SQL. Fingerprint-only rows cannot be explained.',
  explainMonitorPostgresql:
    "Explain from the Monitor is PostgreSQL only; use the editor's Explain for this engine.",
  explain: 'Explain',
  dismissalAppliesView:
    'Dismissal applies for this view; local storage is unavailable.',
  advisorEvidenceUnavailable:
    'Advisor evidence unavailable; start or resume monitoring.',
  diagnosticFailedReview:
    'Diagnostic failed; review the SQL, permissions and context.',
  enabledAppSession:
    'Enabled for this app session. The first run follows the interval; use Run now to start one immediately.',
  scheduleRefusedCheck:
    'Schedule refused. Check provider, telemetry policy, connection and budgets.',
  ruleidVversion: '{ruleId} v{version}',
  none: 'none',
});
