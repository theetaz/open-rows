// SPDX-License-Identifier: Apache-2.0
import type { PaintMetrics } from './model';
/** Shared mutable counters, allocated once per mounted renderer. */
export function paintMetrics(): PaintMetrics {
  return {
    milliseconds: 0,
    placeholders: 0,
    paints: 0,
    scrollHandlerMs: 0,
    reactCommitMs: 0,
    canvasResizeMs: 0,
    canvasResizeEvents: 0,
    prepareMs: 0,
    cellAllocations: 0,
    arrayAllocations: 0,
  };
}
