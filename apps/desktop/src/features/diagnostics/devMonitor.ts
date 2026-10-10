// SPDX-License-Identifier: Apache-2.0
/**
 * Development-only timing monitors (docs/14 §8). `startDevMonitor` is called
 * only in dev builds; nothing here runs in release builds.
 */
import type { UiReport } from '@openrows/ipc';

/** Main-thread tasks at least this long are logged. */
const LONG_TASK_MS = 50;
/** IPC commands at least this slow are logged. */
export const SLOW_IPC_MS = 100;

type Report = (report: UiReport) => void;
type Invoke = (
  command: string,
  args?: unknown,
  options?: unknown,
) => Promise<unknown>;
/** The part of Tauri's runtime that every IPC call goes through. */
type Internals = { invoke: Invoke };

/**
 * Log main-thread tasks over `LONG_TASK_MS` through the `longtask` entries,
 * or, where the engine has none (WebKit), through frame gaps. Returns the stop function.
 */
function watchLongTasks(report: Report): () => void {
  if (observesLongTasks()) {
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        if (entry.duration >= LONG_TASK_MS)
          report({
            kind: 'longTask',
            name: 'task',
            message: null,
            durationMs: entry.duration,
          });
      }
    });
    observer.observe({ type: 'longtask', buffered: true });
    return () => {
      observer.disconnect();
    };
  }
  return watchFrameGaps(report);
}

/**
 * Whether the engine observes `longtask` entries. Some engines define no
 * `PerformanceObserver` or no entry list at all, so both are optional here.
 */
function observesLongTasks(): boolean {
  const observer = globalThis.PerformanceObserver as
    { supportedEntryTypes?: readonly string[] } | undefined;
  return observer?.supportedEntryTypes?.includes('longtask') ?? false;
}

/** A frame arriving late means the main thread was busy; hidden tabs are skipped. */
function watchFrameGaps(report: Report): () => void {
  let last = performance.now();
  let frame = 0;
  const tick = (now: number): void => {
    const gap = now - last;
    if (!document.hidden && gap >= LONG_TASK_MS)
      report({
        kind: 'longTask',
        name: 'frame',
        message: null,
        durationMs: gap,
      });
    last = now;
    frame = requestAnimationFrame(tick);
  };
  frame = requestAnimationFrame(tick);
  return () => {
    cancelAnimationFrame(frame);
  };
}

/** Wrap Tauri's `invoke` so every command slower than `SLOW_IPC_MS` is logged. */
export function timeIpc(internals: Internals, report: Report): void {
  const invoke = internals.invoke.bind(internals);
  const timed: Invoke = (command, args, options) => {
    const started = performance.now();
    return invoke(command, args, options).finally(() => {
      const durationMs = performance.now() - started;
      if (durationMs >= SLOW_IPC_MS)
        report({ kind: 'slowIpc', name: command, message: null, durationMs });
    });
  };
  // New Tauri runtimes freeze invoke. Keep the other monitors and leave it intact.
  Reflect.set(internals, 'invoke', timed);
}

/** Start every development monitor that the running shell supports. */
export function startDevMonitor(report: Report): void {
  watchLongTasks(report);
  const shell = window as Window & { __TAURI_INTERNALS__?: Internals };
  if (shell.__TAURI_INTERNALS__) timeIpc(shell.__TAURI_INTERNALS__, report);
}
