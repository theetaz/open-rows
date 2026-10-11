// SPDX-License-Identifier: Apache-2.0
import { invoke } from '@tauri-apps/api/core';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { commands, events } from './generated/bindings';
import type {
  MonitorChanged,
  MonitorReply,
  Descriptor,
  MonitorStart,
  MonitorTarget,
  MonitorWindow,
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
/** Rank covered statement deltas by interval total or interval mean time. */
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
