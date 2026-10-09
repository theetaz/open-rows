// SPDX-License-Identifier: Apache-2.0
import type { UiReport } from '@openrows/ipc';

/** Reports admitted per window; the rest are dropped so a loop cannot flood the log. */
const REPORT_LIMIT = 30;
/** Length of the rate-limit window (one minute). */
const REPORT_WINDOW_MS = 60_000;

/** Decides whether one more report may be sent now. */
type Admit = () => boolean;

/**
 * A fixed-window limiter: at most `limit` admissions per `windowMs`.
 * The clock is injectable so the window can be tested without waiting.
 */
export function rateLimiter(
  limit: number,
  windowMs: number,
  now: () => number = () => Date.now(),
): Admit {
  let windowStart = Number.NEGATIVE_INFINITY;
  let admitted = 0;
  return () => {
    const time = now();
    if (time - windowStart >= windowMs) {
      windowStart = time;
      admitted = 0;
    }
    if (admitted >= limit) return false;
    admitted += 1;
    return true;
  };
}

/**
 * A function that sends admitted reports to the core log. Reporting must
 * never fail the app, so a failed send is dropped.
 */
export function createReporter(
  send: (report: UiReport) => Promise<void>,
  admit: Admit = rateLimiter(REPORT_LIMIT, REPORT_WINDOW_MS),
): (report: UiReport) => void {
  return (report) => {
    if (!admit()) return;
    send(report).catch(() => undefined);
  };
}
