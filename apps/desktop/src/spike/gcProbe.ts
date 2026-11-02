// SPDX-License-Identifier: Apache-2.0
/** Sentinels live this many frames (≈ 1 s): long enough to leave the young generation. */
const HELD_FRAMES = 60;
/** Finalizations are grouped into windows of this length for the progress line. */
const WINDOW_MS = 100;
/** Timer chain period; a gap far above it means the main thread was blocked. */
const TIMER_MS = 4;
/** Timer gaps reported per scenario, longest first. */
const GAPS = 3;

/**
 * Diagnostic for long grid frames without tracked work. One sentinel per frame
 * is held for about a second, then released: old sentinels are reclaimed only
 * by a full collection, so a burst of their finalizations dates one. A 4 ms
 * timer chain shows whether the main thread itself was blocked (a long gap)
 * or kept running while frames stalled (compositor or display).
 */
export class GcProbe {
  private readonly held: object[] = [];
  private readonly finalized: number[] = [];
  private readonly registry = new FinalizationRegistry<null>(() => {
    this.finalized.push(performance.now());
  });
  private frame = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private started = 0;
  private readonly gaps: { at: number; ms: number }[] = [];

  /** Start one sentinel per animation frame until `stop`; times are relative to this call. */
  start(): void {
    this.started = performance.now();
    const tick = (): void => {
      const sentinel = {};
      this.registry.register(sentinel, null);
      this.held.push(sentinel);
      if (this.held.length > HELD_FRAMES) this.held.shift();
      this.frame = requestAnimationFrame(tick);
    };
    this.frame = requestAnimationFrame(tick);
    let last = performance.now();
    const beat = (): void => {
      const now = performance.now();
      this.gaps.push({ at: last - this.started, ms: now - last });
      this.gaps.sort((a, b) => b.ms - a.ms);
      this.gaps.length = Math.min(this.gaps.length, GAPS);
      last = now;
      this.timer = setTimeout(beat, TIMER_MS);
    };
    this.timer = setTimeout(beat, TIMER_MS);
  }

  stop(): void {
    cancelAnimationFrame(this.frame);
    clearTimeout(this.timer);
  }

  /**
   * `start ms ×count` per window with finalizations, then the longest timer
   * gaps (`gap <ms>@<start ms>`), relative to `start`.
   */
  summary(): string {
    const windows = new Map<number, number>();
    for (const at of this.finalized) {
      const window = Math.floor((at - this.started) / WINDOW_MS) * WINDOW_MS;
      windows.set(window, (windows.get(window) ?? 0) + 1);
    }
    const finalized = [...windows].map(
      ([window, count]) => `${String(window)}ms×${String(count)}`,
    );
    const gaps = this.gaps.map(
      ({ at, ms }) => `gap ${ms.toFixed(0)}@${at.toFixed(0)}`,
    );
    return [...finalized, ...gaps].join(' ');
  }
}
