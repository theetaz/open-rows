// SPDX-License-Identifier: Apache-2.0
import type { PaintMetrics } from '@openrows/grid';
interface CommitTiming {
  begin(): number;
  end(started: number): void;
}
const timing = globalThis as typeof globalThis & {
  __openrowsGridTiming?: CommitTiming;
};
/** Observe complete React commits only during a sampled canvas scenario. */
export function observeReactCommits(metrics: PaintMetrics): () => void {
  const observer: CommitTiming = {
    begin: () => performance.now(),
    end: (started) => {
      metrics.reactCommitMs += performance.now() - started;
    },
  };
  timing.__openrowsGridTiming = observer;
  return () => {
    if (timing.__openrowsGridTiming === observer)
      delete timing.__openrowsGridTiming;
  };
}
