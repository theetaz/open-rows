// SPDX-License-Identifier: Apache-2.0
import { localSetting } from '../settings/localSetting';

/** Rows an automatic `LIMIT` allows; `null` runs `SELECT`s as written. */
type AutoLimit = null | 100 | 200 | 500 | 1000 | 5000 | 10_000;

/** Every automatic limit the settings offer (off by default). */
export const AUTO_LIMITS: readonly AutoLimit[] = [
  null,
  100,
  200,
  500,
  1000,
  5000,
  10_000,
];

/** How runs start (Settings ▸ Execution; `execution` in settings.json). */
export type ExecutionSettings = {
  /** ⌘↵ with text selected runs exactly the selection. */
  runSelection: boolean;
  /**
   * Append `LIMIT n` to every `SELECT` without a limit (dialect-aware, in
   * the core); Run Without Limit skips it once.
   */
  autoLimit: AutoLimit;
  /** Ask before a run of more than one statement starts. */
  confirmMultiple: boolean;
  /** List server notices (and warnings) with the results. */
  showNotices: boolean;
};

export const defaultExecution: ExecutionSettings = {
  runSelection: true,
  autoLimit: null,
  confirmMultiple: false,
  showNotices: true,
};

/** Key in web storage (and `execution` in settings.json). */
export const EXECUTION_KEY = 'openrows.execution';

type Flag = 'runSelection' | 'confirmMultiple' | 'showNotices';

/** Validate stored settings field by field, keeping defaults otherwise. */
export function parseExecution(stored: unknown): ExecutionSettings {
  if (typeof stored !== 'object' || stored === null) return defaultExecution;
  const value = stored as Record<string, unknown>;
  const flag = (key: Flag): boolean =>
    typeof value[key] === 'boolean' ? value[key] : defaultExecution[key];
  const limit = value.autoLimit as AutoLimit;
  return {
    runSelection: flag('runSelection'),
    autoLimit: AUTO_LIMITS.includes(limit) ? limit : defaultExecution.autoLimit,
    confirmMultiple: flag('confirmMultiple'),
    showNotices: flag('showNotices'),
  };
}

export const executionSettings = localSetting(EXECUTION_KEY, parseExecution);

/** Merge a change into the execution settings. */
export function updateExecution(change: Partial<ExecutionSettings>): void {
  executionSettings.set({ ...executionSettings.get(), ...change });
}
