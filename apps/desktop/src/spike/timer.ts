// SPDX-License-Identifier: Apache-2.0
import type { PageTimer } from '@openrows/ipc';

/** Probe consecutive page-clock reads before sampling; null means no observable tick. */
export function measurePageTimer(): PageTimer {
  // Older WebKit can omit this property despite current DOM typings.
  const isolated: unknown = globalThis.crossOriginIsolated;
  let previous = performance.now();
  let resolution = Infinity;
  for (let read = 0; read < 100_000; read++) {
    const current = performance.now();
    const delta = current - previous;
    if (delta > 0) resolution = Math.min(resolution, delta);
    previous = current;
  }
  return {
    resolutionMs: Number.isFinite(resolution) ? resolution : null,
    crossOriginIsolated: isolated === true,
  };
}
