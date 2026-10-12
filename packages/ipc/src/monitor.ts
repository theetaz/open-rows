// SPDX-License-Identifier: Apache-2.0
import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { commands, events } from './generated/bindings';
import type {
  HistoryPoint,
  MonitorChanged,
  MonitorReply,
  Descriptor,
  MonitorStart,
  MonitorTarget,
  MonitorWindow,
  Investigation,
  Suggestion,
  MonitorExplain,
  ExplainReply,
  HypoPgRequest,
  HypoPgReply,
} from './generated/bindings';
import { rawBytes } from './binary';
import { toCommandError, unwrap } from './connections';

/** Explicitly start/resume a shared, bounded collector on a connected profile. */
export function startMonitor(request: MonitorStart): Promise<MonitorReply> {
  return unwrap(commands.monitorStart(request));
}
/** Stop and clear a collector; no monitoring session keeps a profile alive. */
export async function stopMonitor(target: MonitorTarget): Promise<void> {
  await unwrap(commands.monitorStop(target));
}
/** Pause ends the generation; start explicitly re-probes permissions on resume. */
export async function pauseMonitor(target: MonitorTarget): Promise<void> {
  await unwrap(commands.monitorPause(target));
}
/** Read typed scope, requirements and reviewed setup guidance. */
export function monitorCapabilities(
  target: MonitorTarget,
): Promise<Descriptor[]> {
  return unwrap(commands.monitorCapabilities(target));
}
/** Read small source statuses; values are available through Arrow windows. */
export function monitorSnapshot(target: MonitorTarget): Promise<MonitorReply> {
  return unwrap(commands.monitorSnapshot(target));
}
/** Read a bounded Arrow table or the ten-minute scalar live window. */
export function monitorWindow(request: MonitorWindow): Promise<ArrayBuffer> {
  return binary('monitor_window', request);
}
/** Rank covered statement deltas by retained live-window total or weighted mean. */
export function topStatements(request: MonitorWindow): Promise<ArrayBuffer> {
  return binary('monitor_top_statements', request);
}
/** Receive source-status events throttled to one per second. */
export function onMonitorChanged(
  handler: (event: MonitorChanged) => void,
): Promise<UnlistenFn> {
  return events.monitorChanged.listen((event) => {
    handler(event.payload);
  });
}
async function binary(
  command: string,
  request: MonitorWindow,
): Promise<ArrayBuffer> {
  return rawBytes(
    await invoke<unknown>(command, { request }).catch((error: unknown) => {
      throw toCommandError(error);
    }),
  );
}

/** Read bounded aggregate-only history for a profile/database, up to 24 hours. */
export function monitorHistory(
  target: MonitorTarget,
  hours: number,
): Promise<HistoryPoint[]> {
  return unwrap(commands.monitorHistory(target, hours));
}
/** Explicitly enable history profiles; disabling purges synchronously. */
export async function configureMonitorHistory(
  profiles: string[],
): Promise<void> {
  await unwrap(commands.monitorHistoryConfigure(profiles));
}
/** Purge one profile or all local history, without changing collection. */
export async function purgeMonitorHistory(
  profileId: string | null,
): Promise<void> {
  await unwrap(commands.monitorHistoryPurge(profileId));
}

/** Investigate reset-aware deltas and regressions from the native live window. */
export function investigateStatement(
  target: MonitorTarget,
  statementId: string,
): Promise<Investigation> {
  return unwrap(commands.monitorInvestigate(target, statementId));
}
/** Deterministic PostgreSQL advice uses no provider calls. */
export function monitorAdvice(target: MonitorTarget): Promise<Suggestion[]> {
  return unwrap(commands.monitorAdvice(target));
}
/** Revalidate the current evidence and enqueue editor-only SQL. */
export async function proposeMonitorSql(
  target: MonitorTarget,
  suggestionId: string,
): Promise<void> {
  await unwrap(commands.monitorPropose(target, suggestionId));
}
/** A bounded disposable diagnostic; every measured execution confirms natively. */
export function explainMonitorQuery(
  request: MonitorExplain,
): Promise<ExplainReply> {
  return unwrap(commands.monitorExplain(request));
}
/** Opt-in, disposable-session estimated plan comparison for a fresh index proposal. */
export function validateMonitorIndex(
  request: HypoPgRequest,
): Promise<HypoPgReply> {
  return unwrap(commands.monitorHypopg(request));
}
