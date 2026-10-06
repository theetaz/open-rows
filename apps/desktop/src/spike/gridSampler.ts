// SPDX-License-Identifier: Apache-2.0
import { FrameBreakdown } from './frameBreakdown';
import { observeReactCommits } from './reactTiming';
import type { GridRun } from '@openrows/ipc';
import { GridLayout } from '@openrows/grid';
import type { GridHandle, GridDataSource } from '@openrows/grid';
import {
  scenarioDuration,
  continuousSample,
  scrollTarget,
  ROW_HEIGHT,
  PREVIEW_MS,
} from './gridScenarios';
interface Sampling {
  handle: GridHandle;
  source: GridDataSource;
  option: string;
  scenario: string;
  signal: AbortSignal;
}
/** rAF deltas include paint, data arrival, GC and compositor scheduling; no timing samples discarded. */
export function sampleGrid(context: Sampling): Promise<GridRun> {
  return new GridSampler(context).run();
}
class GridSampler {
  private readonly frames = new Float64Array(16_384);
  private readonly paints = new Float64Array(16_384);
  private readonly target = { x: 0, y: 0 };
  private readonly range = { r0: 0, r1: 0, c0: 0, c1: 0 };
  private readonly layout: GridLayout;
  private readonly breakdown: FrameBreakdown;
  private readonly future = { x: 0, y: 0, width: 1040, height: 600 };
  private readonly futureRange = { r0: 0, r1: 0, c0: 0, c1: 0 };
  private nextWindow = -1;
  private commandedAt = 0;
  private readonly requested: number;
  private readonly hits: number;
  private count = 0;
  private blanks = 0;
  private blank = true;
  private started = 0;
  private previous = 0;
  private frame = 0;
  private scrollError = 0;
  private untrack: (() => void) | undefined;
  private resolve: ((report: GridRun) => void) | undefined;
  private reject: ((error: Error) => void) | undefined;
  constructor(private readonly context: Sampling) {
    this.breakdown = new FrameBreakdown(context.handle, context.source);
    this.layout = new GridLayout(context.source.columns, ROW_HEIGHT);
    this.requested = context.source.metrics.requested;
    this.hits = context.source.metrics.hits;
  }
  run(): Promise<GridRun> {
    return new Promise((resolve, reject) => {
      this.untrack = observeReactCommits(this.context.handle.metrics);
      this.resolve = resolve;
      this.reject = reject;
      this.context.signal.addEventListener('abort', this.abort, { once: true });
      if (this.context.signal.aborted) this.abort();
      else this.frame = requestAnimationFrame(this.tick);
    });
  }
  private readonly tick = (now: number): void => {
    if (!this.started) {
      this.started = now;
      this.previous = now;
      this.breakdown.reset();
    } else if (!this.record(now)) return;
    const elapsed = now - this.started;
    const jump = this.context.scenario === 'jump';
    const settled =
      jump &&
      elapsed > PREVIEW_MS &&
      !this.blank &&
      Math.floor(this.context.handle.viewport.y / ROW_HEIGHT) === 999_000;
    if (
      jump &&
      (elapsed > scenarioDuration('jump') ||
        (elapsed === scenarioDuration('jump') && !settled))
    ) {
      this.fail(
        new Error('Jump did not settle without placeholders within 2 seconds'),
      );
      return;
    }
    if (settled || elapsed >= scenarioDuration(this.context.scenario)) {
      this.context.signal.removeEventListener('abort', this.abort);
      this.untrack?.();
      this.resolve?.(this.report(elapsed));
      return;
    }
    const driverStarted = performance.now();
    this.prefetch(elapsed);
    if (elapsed >= PREVIEW_MS)
      scrollTarget(this.context.scenario, elapsed - PREVIEW_MS, this.target);
    this.context.handle.scrollTo(this.target.x, this.target.y);
    this.commandedAt = elapsed;
    this.breakdown.driver(performance.now() - driverStarted);
    this.frame = requestAnimationFrame(this.tick);
  };
  private record(now: number): boolean {
    if (this.count >= this.frames.length) {
      this.fail(new Error('Frame buffer exhausted'));
      return false;
    }
    const { handle, source, scenario } = this.context;
    this.frames[this.count] = now - this.previous;
    this.paints[this.count] = handle.metrics.milliseconds;
    this.layout.visible(this.range, handle.viewport, source.rows);
    this.blank =
      handle.metrics.placeholders > 0 ||
      hasBlank(source, this.range, this.layout.pinned);
    if (this.blank) this.blanks++;
    // Continuous scenarios must follow native/observed logical offsets, not just commanded endpoints.
    if (
      now - this.started > PREVIEW_MS + 100 &&
      (scenario === 'smooth' || scenario === 'horizontal') &&
      continuousSample(scenario, now - this.started - PREVIEW_MS)
    )
      this.scrollError = Math.max(
        this.scrollError,
        Math.abs(handle.viewport.x - this.target.x),
        Math.abs(handle.viewport.y - this.target.y),
      );
    this.breakdown.capture(this.count, this.commandedAt);
    this.count++;
    this.previous = now;
    return true;
  }
  private report(duration: number): GridRun {
    const { option, scenario, source, handle } = this.context;
    const frameMs = Array.from(this.frames.subarray(0, this.count));
    const frameBreakdown = this.breakdown.report(frameMs, scenario);
    return {
      frameBreakdown,
      longestFrames: [...frameBreakdown]
        .sort((a, b) => (b.frameMs ?? 0) - (a.frameMs ?? 0))
        .slice(0, 10),
      option,
      scenario,
      cols: source.columns.length,
      frameMs,
      paintMs: Array.from(this.paints.subarray(0, this.count)),
      blankCellFrames: this.blanks,
      tilesRequested: source.metrics.requested - this.requested,
      cacheHits: source.metrics.hits - this.hits,
      durationMs: duration,
      startRow: 0,
      endRow: Math.floor(handle.viewport.y / ROW_HEIGHT),
      endColumn: this.layout.columnAt(
        handle.viewport.x + handle.viewport.width - 1,
      ),
      maxScrollErrorPx: this.scrollError,
      paintMethod: 'complete layered paint',
    };
  }
  private prefetch(elapsed: number): void {
    // The benchmark uses explicit 100 ms scripted lookahead, without awaiting IPC.
    // This warms teleport destinations too; it is not a claim about predicting user input.
    scrollTarget(
      this.context.scenario,
      Math.max(0, elapsed + 100 - PREVIEW_MS),
      this.future,
    );
    this.future.width = this.context.handle.viewport.width;
    this.future.height = this.context.handle.viewport.height;
    this.layout.visible(
      this.futureRange,
      this.future,
      this.context.source.rows,
    );
    const range = this.futureRange;
    const key =
      (((range.r0 >> 8) * 4096 + (range.r1 >> 8)) * 5 +
        Math.floor(range.c0 / 20)) *
        5 +
      Math.floor(range.c1 / 20);
    if (elapsed < PREVIEW_MS - 100 || key === this.nextWindow) return;
    this.nextWindow = key;
    void this.context.source.window(range, 1).catch((error: unknown) => {
      this.fail(error instanceof Error ? error : new Error(String(error)));
    });
  }
  private readonly abort = (): void => {
    this.fail(new Error('Grid sampling aborted'));
  };
  private fail(error: Error): void {
    this.untrack?.();
    cancelAnimationFrame(this.frame);
    this.context.signal.removeEventListener('abort', this.abort);
    this.reject?.(error);
  }
}
function hasBlank(
  source: GridDataSource,
  range: { r0: number; r1: number; c0: number; c1: number },
  pinned: number,
): boolean {
  for (let row = range.r0 === 0 ? 0 : range.r0 + 1; row < range.r1; row++) {
    for (let col = 0; col <= range.c1; col++)
      if ((col < pinned || col >= range.c0) && !source.cell(row, col))
        return true;
  }
  return false;
}
