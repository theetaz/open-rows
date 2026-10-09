// SPDX-License-Identifier: Apache-2.0
import { openLogsFolder, reportUiEvent } from '@openrows/ipc';
import { forwardErrors } from './forwardErrors';
import { startDevMonitor } from './devMonitor';
import { createReporter } from './reporter';

export { useCrashRecovery } from './useCrashRecovery';

/** Every webview report goes through one rate limit. */
const report = createReporter(reportUiEvent);

/**
 * Forward webview errors to the core log in every build, and start the
 * timing monitors in development builds. Call once, before the first render.
 */
export function installDiagnostics(): void {
  forwardErrors(window, report);
  if (import.meta.env.DEV) startDevMonitor(report);
}

/**
 * Show the logs folder. The promise rejects when the file manager cannot be
 * started; callers decide whether to show that (the menu bar logs it).
 */
export function openLogs(): Promise<void> {
  return openLogsFolder().catch((error: unknown) => {
    report({
      kind: 'error',
      name: 'openLogsFolder',
      message: error instanceof Error ? error.message : String(error),
      durationMs: null,
    });
    throw error;
  });
}
