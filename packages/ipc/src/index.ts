// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type { AppInfo, SpikeReport, SpikeError } from './generated/bindings';

/** Application metadata returned by the Rust shell. */
export type { AppInfo };

/** Read application metadata through the generated command binding. */
export function getAppInfo(): Promise<AppInfo> {
  return commands.appInfo();
}

/** Binary request and typed rejection shape exported from Rust via Specta. */
export type {
  WindowRequest,
  ResultError,
  TileTiming,
  PageTimer,
} from './generated/bindings';

export { resultWindow } from './binary';

/** Atomically persist benchmark samples and exit; available only in spike mode. */
export async function finishSpike(report: SpikeReport): Promise<void> {
  const result = await commands.spikeFinish(report);
  if (result.status === 'error') throw spikeFailure(result.error);
}

/** Query spike mode and show the native window before the worker starts. */
export async function startSpike(): Promise<boolean> {
  if (!isTauri()) return false;
  const result = await commands.spikeStart();
  if (result.status === 'error') throw spikeFailure(result.error);
  return result.data;
}

/** Append one diagnostic line to the benchmark progress channel. */
export async function spikeProgress(
  stage: string,
  detail: string,
): Promise<void> {
  const result = await commands.spikeProgress(stage, detail);
  if (result.status === 'error') throw spikeFailure(result.error);
}

function spikeFailure(error: SpikeError): Error {
  switch (error.kind) {
    case 'failed':
      return new Error(error.message);
    case 'disabled':
      return new Error('IPC spike mode is disabled');
    case 'missingPath':
      return new Error('OPENROWS_SPIKE_REPORT is required');
  }
}
