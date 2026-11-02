// SPDX-License-Identifier: Apache-2.0
import { isTauri } from '@tauri-apps/api/core';
import { commands } from './generated/bindings';
import type { SpikeError, SpikeReport } from './generated/bindings';

/** Atomically persist benchmark samples and exit; available only in spike mode. */
export async function finishSpike(report: SpikeReport): Promise<void> {
  const result = await commands.spikeFinish(report);
  if (result.status === 'error') throw spikeFailure(result.error);
}

/** Query spike mode and show the native window before the worker starts. */
export async function startSpike(): Promise<false | SpikeMode> {
  if (!isTauri()) return false;
  const result = await commands.spikeStart();
  if (result.status === 'error') throw spikeFailure(result.error);
  if (result.data === null) return false;
  const mode = SPIKE_MODES.find((known) => known === result.data);
  if (mode) return mode;
  throw new Error('Unknown spike mode');
}

/** Release measurement modes; `connected` and `monitor` stay open for bench-memory. */
const SPIKE_MODES = ['ipc', 'grid', 'editor', 'connected', 'monitor'] as const;
export type SpikeMode = (typeof SPIKE_MODES)[number];

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
