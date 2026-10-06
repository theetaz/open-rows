// SPDX-License-Identifier: Apache-2.0
import type { GridFrame } from '@openrows/ipc';
import type { GridHandle, GridDataSource } from '@openrows/grid';
import { scenarioStep } from './gridScenarios';
const RENDER_FIELDS = [
  'scrollHandlerMs',
  'reactCommitMs',
  'canvasResizeMs',
  'canvasResizeEvents',
  'prepareMs',
  'milliseconds',
  'arrayAllocations',
  'cellAllocations',
] as const;
const SOURCE_FIELDS = [
  'tileApplyMs',
  'tileConvertMs',
  'workerConvertMs',
  'tileAllocations',
  'arrayAllocations',
  'cellAllocations',
] as const;
const STRIDE = RENDER_FIELDS.length + SOURCE_FIELDS.length + 2;
/** Flat preallocated storage; snapshot and counter reset allocate nothing in a sampled frame. */
export class FrameBreakdown {
  private readonly values = new Float64Array(16_384 * STRIDE);
  private driverMs = 0;
  constructor(
    private readonly handle: GridHandle,
    private readonly source: GridDataSource,
  ) {}
  driver(milliseconds: number): void {
    this.driverMs += milliseconds;
  }
  capture(index: number, elapsed: number): void {
    let offset = index * STRIDE;
    for (const field of RENDER_FIELDS)
      this.values[offset++] = this.handle.metrics[field];
    for (const field of SOURCE_FIELDS)
      this.values[offset++] = this.source.metrics[field];
    this.values[offset++] = this.driverMs;
    this.values[offset] = elapsed;
    this.reset();
  }
  reset(): void {
    for (const field of RENDER_FIELDS) this.handle.metrics[field] = 0;
    for (const field of SOURCE_FIELDS) this.source.metrics[field] = 0;
    this.handle.metrics.placeholders = 0;
    this.handle.metrics.paints = 0;
    this.driverMs = 0;
  }
  report(frames: readonly number[], scenario: string): GridFrame[] {
    return frames.map((frameMs, index) =>
      this.frame({ index, frameMs, scenario }),
    );
  }
  private frame({
    index,
    frameMs,
    scenario,
  }: {
    index: number;
    frameMs: number;
    scenario: string;
  }): GridFrame {
    const offset = index * STRIDE;
    const at = (slot: number): number => this.values[offset + slot] ?? 0;
    return {
      index,
      frameMs,
      step: scenarioStep(scenario, at(15)),
      scrollHandlerMs: at(0),
      reactCommitMs: at(1),
      canvasResizeMs: at(2),
      canvasResizeEvents: at(3),
      prepareMs: at(4),
      paintMs: at(5),
      tileApplyMs: at(8),
      tileConvertMs: at(9),
      workerConvertMs: at(10),
      tileAllocations: at(11),
      arrayAllocations: at(6) + at(12),
      cellAllocations: at(7) + at(13),
      driverMs: at(14),
    };
  }
}
