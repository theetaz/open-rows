// SPDX-License-Identifier: Apache-2.0
import { localSetting } from './localSetting';

/** Log levels the core accepts, least to most verbose (docs/14 §6). */
export const LOG_LEVELS = ['error', 'warn', 'info', 'debug', 'trace'] as const;
export type LogLevel = (typeof LOG_LEVELS)[number];

/** Diagnostics preferences; `settings.json` keeps them under `diagnostics`. */
type DiagnosticsSettings = {
  /** What the log file records. Applies at the next launch; `OPENROWS_LOG` overrides it. */
  logLevel: LogLevel;
  /**
   * Opt-in crash reports. A stub: OpenRows sends no telemetry and no crash
   * reports in any case, and this switch only stores the choice (docs/14 §7).
   */
  crashReports: boolean;
};

const defaultDiagnostics: DiagnosticsSettings = {
  logLevel: 'info',
  crashReports: false,
};

/** Key in web storage (and `diagnostics` in settings.json). */
export const DIAGNOSTICS_KEY = 'openrows.diagnostics';

/** Validate stored settings field by field, keeping defaults otherwise. */
export function parseDiagnostics(stored: unknown): DiagnosticsSettings {
  if (typeof stored !== 'object' || stored === null) return defaultDiagnostics;
  const value = stored as Record<string, unknown>;
  const logLevel = LOG_LEVELS.find((level) => level === value.logLevel);
  return {
    logLevel: logLevel ?? defaultDiagnostics.logLevel,
    crashReports:
      typeof value.crashReports === 'boolean'
        ? value.crashReports
        : defaultDiagnostics.crashReports,
  };
}

export const diagnosticsSettings = localSetting(
  DIAGNOSTICS_KEY,
  parseDiagnostics,
);

/** Merge a change into the diagnostics settings. */
export function updateDiagnostics(change: Partial<DiagnosticsSettings>): void {
  diagnosticsSettings.set({ ...diagnosticsSettings.get(), ...change });
}
